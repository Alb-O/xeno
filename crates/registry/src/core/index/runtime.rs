//! Registry definition indexing and runtime extension system.
//!
//! # Purpose
//!
//! - Owns: Subsystem definition indexing, runtime extension registration, and collision/winner resolution.
//! - Does not own: Command execution, hook emission (owns the definitions only).
//! - Source of truth: [`RegistryDb`] and its domain-specific [`RuntimeRegistry`] instances (storing [`Snapshot<T>`]).
//!
//! # Mental model
//!
//! - Terms: Builtin (compile-time `&'static T`), Owned (runtime `Arc<T>`), Snapshot (atomic view),
//!   DefPtr (thin pointer into either), RegistryRef (snapshot-pinned guard), Winner (conflict resolution),
//!   Eviction (cleanup after override).
//! - Lifecycle in one sentence: A build-time index is wrapped in an `ArcSwap` snapshot; runtime
//!   extensions are `Arc`-owned inside the snapshot (no `Box::leak`), and lookups return
//!   [`RegistryRef<T>`] guards that pin the snapshot alive while the caller holds a reference.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`DefPtr<T>`] | Thin, `Copy` pointer to a definition | MUST NOT outlive its backing storage (`&'static` or `Arc` in snapshot) | `DefPtr::from_ref` |
//! | [`RegistryRef<T>`] | Guard that pins a snapshot `Arc` while exposing `&T` via `Deref` | MUST hold `Arc<Snapshot>` for the lifetime of the borrow | `RuntimeRegistry::get`, `RuntimeRegistry::all` |
//! | [`Snapshot<T>`] | Atomic view of registry; owns runtime `Arc<T>` definitions in `owned` | Immutable after construction, lock-free read | `RuntimeRegistry` CAS loop |
//! | [`RegistryIndex<T>`] | Build-time index of `DefPtr<T>` (builtins only) | All pointers are `&'static` | `RegistryBuilder::build` |
//! | [`KeyStore`] | Abstract index mutation | MUST implement `evict_def` | `SnapshotStore` / `BuildStore` |
//! | [`DuplicatePolicy`] | Conflict resolution rule | MUST be deterministic | `RuntimeRegistry::with_policy` |
//! | [`InsertAction`] | Outcome of registration | Informs diagnostics | `insert_typed_key` |
//! | [`KeyKind`] | Type of registration key | Id, Name, or Alias | `RegistryMeta` |
//!
//! # Invariants
//!
//! - MUST keep ID lookup unambiguous (one winner per ID).
//!   - Enforced in: `insert_typed_key` (build-time), `insert_id_key_runtime` (runtime).
//!   - Tested by: `core::index::tests::test_id_first_lookup`
//!   - Failure symptom: Panics during build/startup or stale name lookups after override.
//! - MUST evict old definitions on ID override.
//!   - Enforced in: `insert_id_key_runtime` (calls `evict_def`).
//!   - Tested by: `core::index::tests::test_id_override_eviction`
//!   - Failure symptom: Stale name/alias lookups pointing to a replaced definition.
//! - MUST maintain stable numeric IDs for builtin actions.
//!   - Enforced in: `RegistryDbBuilder::build`.
//!   - Tested by: TODO (add regression: test_stable_action_ids)
//!   - Failure symptom: Inconsistent `ActionId` mappings in optimized input handling.
//! - Owned runtime definitions MUST be kept alive by `Snapshot::owned` for as long as any
//!   `DefPtr` in the snapshot's index maps references them.
//!   - Enforced in: `RuntimeRegistry::try_register_many_internal` (extends `owned`, prunes unreferenced).
//!   - Tested by: TODO (add regression: test_owned_defs_survive_snapshot_clone)
//!   - Failure symptom: Use-after-free when dereferencing a `RegistryRef` whose backing `Arc` was pruned.
//!
//! # Data flow
//!
//! - Builtins: `inventory` or explicit registration builds base index via `RegistryBuilder`.
//!   Pointers are `&'static T` wrapped in `DefPtr`.
//! - Plugins: Sorted by priority, executed to mutate the builder.
//! - Snapshot: `RuntimeRegistry` loads built index into a `Snapshot` (no owned defs yet).
//! - Mutation: `register`/`register_owned` clones snapshot, wraps owned defs in `Arc<T>`,
//!   applies changes via `insert_id_key_runtime`, prunes unreferenced `Arc`s, and CAS-updates.
//! - Resolution: `get(key)` loads the snapshot `Arc`, looks up `DefPtr`, returns a
//!   `RegistryRef` that holds the snapshot alive.
//!
//! # Lifecycle
//!
//! - Build Phase: Builtins registered; plugins run; index finalized.
//! - Runtime Phase: Snapshots loaded; runtime registration allowed; lookups lock-free.
//!   Callers hold `RegistryRef` guards; the snapshot is dropped when all guards are released.
//!
//! # Concurrency and ordering
//!
//! - Lock-free reads: `snap.load_full()` returns an `Arc<Snapshot>` without blocking.
//! - CAS Retry Loop: Writes retry if the snapshot changed during mutation.
//! - Deterministic Winners: `DuplicatePolicy` ensures the same definition wins regardless of registration order.
//! - Snapshot Lifetime: `RegistryRef` prevents premature drop of the snapshot (and its owned `Arc<T>` defs).
//!
//! # Failure modes and recovery
//!
//! - Duplicate Build ID: Panic during startup to prevent ambiguous behavior.
//! - CAS Contention: Writes may take multiple retries under heavy concurrent registration.
//! - Eviction Failure: If not implemented for a new index kind, stale definitions persist.
//!
//! # Recipes
//!
//! ## Override a builtin definition
//!
//! Steps:
//! - Call `registry.try_register_override(new_def)`.
//! - Ensure `new_def.meta().priority` is higher than the existing one if using `ByPriority`.
//!
//! ## Register an owned (non-static) definition
//!
//! Steps:
//! - Call `registry.register_owned(def)` or `registry.register_many_owned(defs)`.
//! - The definition is wrapped in `Arc<T>` and stored in `Snapshot::owned`.
//! - No `Box::leak`; the `Arc` is dropped when the snapshot is replaced and all
//!   `RegistryRef` guards have been released.
//!
use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;

use super::collision::{ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore};
use super::insert::{insert_id_key_runtime, insert_typed_key};
use super::types::{DefPtr, Map, RegistryIndex};
use crate::RegistryEntry;
use crate::error::{InsertAction, RegistryError};

/// Snapshot-pinning guard that provides `&T` access to a registry definition.
///
/// Holds an `Arc<Snapshot<T>>` to keep the snapshot (and any `Arc<T>` owned defs
/// within it) alive for the lifetime of this guard. Dereferences to `&T` via
/// the internal [`DefPtr`].
///
/// Returned by [`RuntimeRegistry::get`] and [`RuntimeRegistry::all`]. Cloning
/// is cheap (Arc bump + pointer copy).
pub struct RegistryRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	snap: Arc<Snapshot<T>>,
	ptr: DefPtr<T>,
}

impl<T> Clone for RegistryRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			ptr: self.ptr,
		}
	}
}

impl<T> std::ops::Deref for RegistryRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	type Target = T;

	fn deref(&self) -> &T {
		// Safety: The definition is kept alive by the snapshot Arc held in the guard.
		unsafe { self.ptr.as_ref() }
	}
}

/// Single source of truth for registry lookups.
///
/// Contains a merged view of builtins and runtime extensions.
pub struct Snapshot<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	pub by_id: Map<Box<str>, DefPtr<T>>,
	pub by_key: Map<Box<str>, DefPtr<T>>,
	pub items_all: Vec<DefPtr<T>>,
	pub items_effective: Vec<DefPtr<T>>,
	/// Owns runtime-registered definitions so their pointers stay valid.
	pub owned: Vec<Arc<T>>,
	pub collisions: Vec<Collision>,
}

impl<T> Clone for Snapshot<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn clone(&self) -> Self {
		Self {
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			items_all: self.items_all.clone(),
			items_effective: self.items_effective.clone(),
			owned: self.owned.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T> Snapshot<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	/// Creates a new snapshot from a builtin index.
	fn from_builtins(b: &RegistryIndex<T>) -> Self {
		Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			items_all: b.items_all.to_vec(),
			items_effective: b.items_effective.to_vec(),
			owned: Vec::new(),
			collisions: b.collisions.to_vec(),
		}
	}

	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get_ptr(&self, key: &str) -> Option<DefPtr<T>> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id_ptr(&self, id: &str) -> Option<DefPtr<T>> {
		self.by_id.get(id).copied()
	}
}

/// Registry wrapper for runtime-extensible registries.
pub struct RuntimeRegistry<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	pub(super) label: &'static str,
	pub(super) builtins: RegistryIndex<T>,
	pub(super) snap: ArcSwap<Snapshot<T>>,
	pub(super) policy: DuplicatePolicy,
}

impl<T> RuntimeRegistry<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T>) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self {
			label,
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy: DuplicatePolicy::for_build(),
		}
	}

	/// Creates a new runtime registry with a custom duplicate policy.
	pub fn with_policy(
		label: &'static str,
		builtins: RegistryIndex<T>,
		policy: DuplicatePolicy,
	) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self {
			label,
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy,
		}
	}

	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<RegistryRef<T>> {
		let snap = self.snap.load_full();
		let ptr = snap.get_ptr(key)?;
		Some(RegistryRef { snap, ptr })
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<RegistryRef<T>> {
		let snap = self.snap.load_full();
		let ptr = snap.get_by_id_ptr(id)?;
		Some(RegistryRef { snap, ptr })
	}

	/// Registers a definition at runtime.
	pub fn register(&self, def: &'static T) -> bool {
		self.try_register(def).is_ok()
	}

	/// Registers an owned definition without leaking.
	pub fn register_owned(&self, def: T) -> bool {
		self.try_register_owned(def).is_ok()
	}

	/// Registers many definitions at runtime in a single atomic operation.
	pub fn register_many<I>(&self, defs: I) -> Result<usize, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		Ok(self.try_register_many(defs)?.len())
	}

	/// Registers many owned definitions without leaking.
	pub fn register_many_owned<I>(&self, defs: I) -> Result<usize, RegistryError>
	where
		I: IntoIterator<Item = T>,
	{
		Ok(self.try_register_many_owned(defs)?.len())
	}

	/// Attempts to register many definitions at runtime in a single atomic operation.
	pub fn try_register_many<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		self.try_register_many_internal(defs.into_iter().map(DefPtr::from_ref), Vec::new(), false)
	}

	/// Attempts to register many owned definitions without leaking.
	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = T>,
	{
		let owned: Vec<Arc<T>> = defs.into_iter().map(Arc::new).collect();
		let ptrs: Vec<DefPtr<T>> = owned.iter().map(|a| DefPtr::from_ref(&**a)).collect();
		self.try_register_many_internal(ptrs, owned, false)
	}

	/// Attempts to register many definitions with ID override support.
	pub fn try_register_many_override<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		self.try_register_many_internal(defs.into_iter().map(DefPtr::from_ref), Vec::new(), true)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		new_owned: Vec<Arc<T>>,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefPtr<T>>,
	{
		let input_defs: Vec<DefPtr<T>> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			// Build pointer set of already registered items for efficient dedup
			let mut existing_ptrs: rustc_hash::FxHashSet<DefPtr<T>> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &item in &next.items_all {
				existing_ptrs.insert(item);
			}

			let mut new_defs_indices = Vec::with_capacity(input_defs.len());
			for (idx, &def) in input_defs.iter().enumerate() {
				if !existing_ptrs.contains(&def) {
					new_defs_indices.push(idx);
				}
			}

			if new_defs_indices.is_empty() {
				return Ok(vec![InsertAction::KeptExisting; input_defs.len()]);
			}

			let mut actions = vec![InsertAction::KeptExisting; input_defs.len()];
			let choose_winner = self.make_choose_winner();

			{
				let mut store = SnapshotStore { snap: &mut next };

				for idx in new_defs_indices {
					let def = input_defs[idx];
					let meta = unsafe { def.as_ref() }.meta();

					let id_action = if allow_overrides {
						insert_id_key_runtime(&mut store, self.label, choose_winner, meta.id, def)?
					} else {
						insert_typed_key(
							&mut store,
							self.label,
							choose_winner,
							KeyKind::Id,
							meta.id,
							def,
						)?
					};

					// If we're overriding and we lost the ID contest, skip this item entirely
					if allow_overrides && id_action == InsertAction::KeptExisting {
						actions[idx] = InsertAction::KeptExisting;
						continue;
					}

					let action = insert_typed_key(
						&mut store,
						self.label,
						choose_winner,
						KeyKind::Name,
						meta.name,
						def,
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							self.label,
							choose_winner,
							KeyKind::Alias,
							alias,
							def,
						)?;
					}

					store.snap.items_all.push(def);
					actions[idx] = action;
				}
			}

			// Add successfully registered owned items
			next.owned.extend(new_owned.clone());

			// Prune unreferenced owned definitions
			{
				let mut referenced = rustc_hash::FxHashSet::default();
				for &ptr in next.by_id.values() {
					referenced.insert(ptr);
				}
				for &ptr in next.by_key.values() {
					referenced.insert(ptr);
				}
				next.owned
					.retain(|arc| referenced.contains(&DefPtr::from_ref(&**arc)));
			}

			// Update items_effective
			let mut effective_set: rustc_hash::FxHashSet<DefPtr<T>> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &def in next.by_id.values() {
				effective_set.insert(def);
			}
			for &def in next.by_key.values() {
				effective_set.insert(def);
			}
			next.items_effective = next
				.items_all
				.iter()
				.copied()
				.filter(|d| effective_set.contains(d))
				.collect();

			let next_arc = Arc::new(next);
			let prev = self.snap.compare_and_swap(&cur, next_arc);

			if Arc::ptr_eq(&prev, &cur) {
				return Ok(actions);
			}
			// CAS failed, retry
		}
	}

	/// Attempts to register a definition at runtime, returning detailed error info.
	pub fn try_register(&self, def: &'static T) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many(std::iter::once(def))?[0])
	}

	/// Attempts to register an owned definition without leaking.
	pub fn try_register_owned(&self, def: T) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many_owned(std::iter::once(def))?[0])
	}

	/// Attempts to register a definition with ID override support.
	pub fn try_register_override(&self, def: &'static T) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many_override(std::iter::once(def))?[0])
	}

	fn make_choose_winner(&self) -> ChooseWinner<T> {
		match self.policy {
			DuplicatePolicy::Panic => |kind, key, existing, new| {
				panic!(
					"runtime registry key conflict: kind={} key={:?} existing_id={} new_id={}",
					kind,
					key,
					existing.id(),
					new.id()
				);
			},
			DuplicatePolicy::FirstWins => |_, _, _, _| false,
			DuplicatePolicy::LastWins => |_, _, _, _| true,
			DuplicatePolicy::ByPriority => {
				|_, _, existing, new| new.total_order_cmp(existing) == Ordering::Greater
			}
		}
	}

	/// Returns the number of unique definitions (builtins + extras).
	pub fn len(&self) -> usize {
		self.snap.load().items_effective.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.snap.load().items_effective.is_empty()
	}

	/// Returns all definitions (builtins followed by extras).
	pub fn all(&self) -> Vec<RegistryRef<T>> {
		let snap = self.snap.load_full();
		snap.items_effective
			.iter()
			.map(|&ptr| RegistryRef {
				snap: snap.clone(),
				ptr,
			})
			.collect()
	}

	/// Returns an iterator over effective definitions.
	pub fn iter(&self) -> Vec<RegistryRef<T>> {
		self.all()
	}

	/// Returns all effective definitions.
	pub fn items(&self) -> Vec<RegistryRef<T>> {
		self.all()
	}

	/// Returns all recorded collisions (builtins + runtime).
	pub fn collisions(&self) -> Vec<Collision> {
		self.snap.load().collisions.clone()
	}

	/// Returns the underlying builtins index.
	pub fn builtins(&self) -> &RegistryIndex<T> {
		&self.builtins
	}

	/// Returns the current snapshot guard so callers can read without allocating.
	pub fn snapshot(&self) -> Arc<Snapshot<T>> {
		self.snap.load_full()
	}

	/// Executes a closure while the snapshot guard is alive.
	pub fn with_snapshot<R>(&self, f: impl FnOnce(&Snapshot<T>) -> R) -> R {
		let snap = self.snap.load();
		f(&snap)
	}
}

/// KeyStore over Snapshot for shared insertion logic.
struct SnapshotStore<'a, T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	snap: &'a mut Snapshot<T>,
}

impl<T> KeyStore<T> for SnapshotStore<'_, T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn get_id_owner(&self, id: &str) -> Option<DefPtr<T>> {
		self.snap.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefPtr<T>> {
		self.snap.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &str, def: DefPtr<T>) {
		self.snap.by_key.insert(Box::from(key), def);
	}

	fn insert_id(&mut self, id: &str, def: DefPtr<T>) -> Option<DefPtr<T>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefPtr<T>) {
		self.snap.by_id.insert(Box::from(id), def);
	}

	fn evict_def(&mut self, def: DefPtr<T>) {
		self.snap.by_key.retain(|_, &mut v| !v.ptr_eq(def));
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
