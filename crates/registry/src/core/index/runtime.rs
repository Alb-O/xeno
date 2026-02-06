//! Registry definition indexing and runtime extension system.
//!
//! # Purpose
//!
//! - Owns: definition indexing (ID/name/alias), runtime extension registration, collision reporting,
//!   deterministic iteration order, and winner resolution policy.
//! - Does not own: command execution, hook emission, or any subsystem behavior beyond selecting which
//!   definition is visible under a given key.
//! - Source of truth: [`RegistryDb`] and its domain-specific [`RuntimeRegistry`] instances, each
//!   storing an atomic [`Snapshot<T>`].
//!
//! # Mental model
//!
//! Think of a registry as an *append-only stream of candidate definitions* with a *current visible
//! winner* for each key.
//!
//! - **Builtin** definitions live for the process (`&'static T`).
//! - **Owned** definitions are runtime-registered (`Arc<T>`).
//! - A [`Snapshot<T>`] is an immutable, atomic view used for lock-free reads.
//! - Definitions inside a snapshot are referenced by [`DefRef<T>`], a safe handle:
//!   `Builtin(&'static T) | Owned(Arc<T>)`.
//! - Lookup keys are split into:
//!   - `by_id`: the single winner per stable ID
//!   - `by_key`: winners for names/aliases (and other non-ID keys)
//! - [`id_order`] preserves deterministic iteration order of *IDs that exist* in the snapshot.
//!   Iteration is driven by `id_order` and resolved through `by_id`.
//!
//! **Shift from `DefPtr` to `DefRef`:**
//! Earlier implementations stored raw pointers (`DefPtr`) plus a separate `owned` list and a manual
//! prune pass to keep pointed-to allocations alive. This module now stores `DefRef` handles directly
//! in the indices. Lifetime is structural: if a `DefRef::Owned(Arc<T>)` is reachable from `by_id` or
//! `by_key`, the allocation stays alive; eviction naturally drops the last `Arc` when no longer
//! referenced by the snapshot (and when no guards remain).
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`DefRef<T>`] | Safe handle to a definition (`Builtin`/`Owned`) | Owned refs keep their allocation alive via `Arc` reachability | `DefRef::Builtin`, `DefRef::Owned` |
//! | [`RegistryRef<T>`] | Guard that pins an `Arc<Snapshot<T>>` while exposing `&T` via `Deref` | MUST hold the snapshot `Arc` for the lifetime of the borrow | [`RuntimeRegistry::get`], [`RuntimeRegistry::all`] |
//! | [`Snapshot<T>`] | Atomic view of the registry indices | Immutable after construction; shared via `ArcSwap` | `RuntimeRegistry` CAS loop |
//! | [`RegistryIndex<T>`] | Build-time index of builtins (seed snapshot) | Builtins are `&'static` | `RegistryBuilder::build` |
//! | [`KeyStore`] | Abstract mutation surface used by insertion logic | MUST implement `evict_def` for runtime overrides | `SnapshotStore` / build store |
//! | [`DuplicatePolicy`] | Winner selection rule | MUST be deterministic for reproducible iteration/results | [`RuntimeRegistry::with_policy`] |
//! | [`InsertAction`] | Outcome classification for insertion | Used for diagnostics and tests | `insert_typed_key`, `insert_id_key_runtime` |
//! | [`KeyKind`] | Class of key being inserted | `Id`, `Name`, `Alias` | `RegistryMeta` |
//!
//! # Invariants
//!
//! - MUST keep ID lookup unambiguous (one winner per ID).
//!   - Enforced in: `insert_typed_key` (build-time), `insert_id_key_runtime` (runtime).
//!   - Tested by: `core::index::tests::test_id_first_lookup`
//!   - Failure symptom: Panics during build/startup or inconsistent `get_by_id` results.
//!
//! - MUST evict old definitions on ID override (including name/alias winners pointing to the old def).
//!   - Enforced in: `insert_id_key_runtime` (calls `KeyStore::evict_def` before `set_id_owner`).
//!   - Tested by: `core::index::tests::test_id_override_eviction`
//!   - Failure symptom: stale name/alias lookups returning a definition that is no longer the ID owner.
//!
//! - MUST maintain deterministic iteration order over effective definitions.
//!   - Enforced in: `RuntimeRegistry::try_register_many_internal` (pushes to `id_order` only on
//!     `InsertAction::InsertedNew`; replacements do not reorder).
//!   - Tested by: `core::index::tests::test_override_preserves_order` and existing ID
//!     ordering tests that assume stable traversal.
//!   - Failure symptom: nondeterministic `all()` / `iter()` ordering across runs or after overrides.
//!
//! - Owned runtime definitions MUST remain alive for as long as they are reachable from the snapshot.
//!   - Enforced in: structure (reachability) — `Snapshot` stores `DefRef::Owned(Arc<T>)` directly in
//!     `by_id` / `by_key`, and [`RegistryRef<T>`] pins the snapshot `Arc` while borrowing `&T`.
//!   - Tested by: `core::index::tests::test_uaf_trace_elimination`.
//!   - Failure symptom: use-after-free when dereferencing a `RegistryRef` after concurrent writes.
//!
//! # Data flow
//!
//! - Builtins:
//!   - `RegistryBuilder` constructs a [`RegistryIndex<T>`] from `&'static T` definitions.
//!   - The runtime registry seeds its first [`Snapshot<T>`] from that builtins index.
//! - Runtime registration:
//!   - `register` / `register_owned` call `try_register_many_internal`.
//!   - Writes clone the current snapshot, apply insertions via `insert_typed_key` / `insert_id_key_runtime`
//!     against a `KeyStore`, then CAS-publish the new snapshot.
//! - Dedup/admission gating:
//!   - Registration skips definitions whose identity is already present in the snapshot (and within the
//!     same batch), avoiding duplicate `id_order` entries and redundant collisions.
//!   - In override mode, definitions that lose the ID contest are not admitted via name/alias.
//! - Lookups:
//!   - `get(key)` loads the snapshot `Arc` and returns a [`RegistryRef<T>`] holding a cloned `DefRef<T>`.
//!   - `all()` iterates `id_order`, resolves each ID via `by_id`, and returns guards in stable order.
//!
//! # Lifecycle
//!
//! - Build phase:
//!   - Builtins registered; plugins may mutate the builder; duplicate policy resolves collisions.
//!   - A finalized [`RegistryIndex<T>`] is produced.
//! - Runtime phase:
//!   - [`RuntimeRegistry<T>`] creates an `ArcSwap<Snapshot<T>>` seeded from builtins.
//!   - Runtime registrations CAS-publish new snapshots; reads stay lock-free.
//!   - Snapshots are dropped when replaced and when no [`RegistryRef<T>`] guards remain.
//!
//! # Concurrency and ordering
//!
//! - Lock-free reads: `snap.load_full()` returns an `Arc<Snapshot<T>>` without blocking.
//! - CAS retry loop: writers retry if a concurrent write published a newer snapshot.
//! - Deterministic winners: [`DuplicatePolicy`] must be deterministic; `ByPriority` uses a total order
//!   comparison and breaks ties using existing-wins semantics.
//! - Snapshot lifetime: [`RegistryRef<T>`] prevents the snapshot (and any `Arc<T>` held by `DefRef`) from
//!   being dropped while callers hold borrowed references.
//!
//! # Failure modes and recovery
//!
//! - Duplicate build ID: build-time insertion panics to prevent ambiguous behavior.
//! - CAS contention: writes may retry under heavy concurrent registration.
//! - Eviction failure: if `evict_def` is not implemented for a `KeyStore`, stale keys can persist.
//!
//! # Recipes
//!
//! ## Override a builtin definition
//!
//! - Call `registry.try_register_override(new_def)`.
//! - Ensure `new_def.meta().priority` is higher than the existing one if using `ByPriority`.
//!
//! ## Register an owned (non-static) definition
//!
//! - Call `registry.register_owned(def)` or `registry.register_many_owned(defs)`.
//! - The definition is wrapped in `Arc<T>` and stored in-index as `DefRef::Owned`.
//! - The `Arc` is dropped when the definition becomes unreachable from the latest snapshot and all
//!   snapshot guards have been released.
//!
use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;

use super::collision::{ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore};
use super::insert::{insert_id_key_runtime, insert_typed_key};
use super::types::{DefRef, Map, RegistryIndex};
use crate::RegistryEntry;
use crate::error::{InsertAction, RegistryError};
/// Marker trait for types that can be stored in a runtime registry.
///
/// Combines `RegistryEntry` with thread-safety and static lifetime requirements.
pub trait RuntimeEntry: RegistryEntry + Send + Sync + 'static {}

impl<T> RuntimeEntry for T where T: RegistryEntry + Send + Sync + 'static {}

/// Snapshot-pinning guard that provides `&T` access to a registry definition.
///
/// Holds an `Arc<Snapshot<T>>` to keep the snapshot (and any `Arc<T>` owned defs
/// within it) alive for the lifetime of this guard. Dereferences to `&T` via
/// the internal [`DefRef`].
///
/// Returned by [`RuntimeRegistry::get`] and [`RuntimeRegistry::all`]. Cloning
/// is cheap (Arc bump + variant copy).
pub struct RegistryRef<T>
where
	T: RuntimeEntry,
{
	snap: Arc<Snapshot<T>>,
	def: DefRef<T>,
}

impl<T> Clone for RegistryRef<T>
where
	T: RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			def: self.def.clone(),
		}
	}
}

impl<T> std::ops::Deref for RegistryRef<T>
where
	T: RuntimeEntry,
{
	type Target = T;

	fn deref(&self) -> &T {
		self.def.as_entry()
	}
}

/// Single source of truth for registry lookups.
///
/// Contains a merged view of builtins and runtime extensions.
pub struct Snapshot<T>
where
	T: RuntimeEntry,
{
	pub by_id: Map<Box<str>, DefRef<T>>,
	pub by_key: Map<Box<str>, DefRef<T>>,
	/// Explicit iteration order for effective definitions.
	pub id_order: Vec<Box<str>>,
	pub collisions: Vec<Collision>,
}

impl<T> Clone for Snapshot<T>
where
	T: RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			id_order: self.id_order.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T> Snapshot<T>
where
	T: RuntimeEntry,
{
	/// Creates a new snapshot from a builtin index.
	fn from_builtins(b: &RegistryIndex<T>) -> Self {
		Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			id_order: b.id_order.clone(),
			collisions: b.collisions.to_vec(),
		}
	}

	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get_def(&self, key: &str) -> Option<DefRef<T>> {
		self.by_id
			.get(key)
			.cloned()
			.or_else(|| self.by_key.get(key).cloned())
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id_def(&self, id: &str) -> Option<DefRef<T>> {
		self.by_id.get(id).cloned()
	}
}

/// Registry wrapper for runtime-extensible registries.
pub struct RuntimeRegistry<T>
where
	T: RuntimeEntry,
{
	pub(super) label: &'static str,
	pub(super) builtins: RegistryIndex<T>,
	pub(super) snap: ArcSwap<Snapshot<T>>,
	pub(super) policy: DuplicatePolicy,
}

impl<T> RuntimeRegistry<T>
where
	T: RuntimeEntry,
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
		let def = snap.get_def(key)?;
		Some(RegistryRef { snap, def })
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<RegistryRef<T>> {
		let snap = self.snap.load_full();
		let def = snap.get_by_id_def(id)?;
		Some(RegistryRef { snap, def })
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
		self.try_register_many_internal(defs.into_iter().map(DefRef::Builtin), false)
	}

	/// Attempts to register many owned definitions without leaking.
	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = T>,
	{
		self.try_register_many_internal(defs.into_iter().map(|d| DefRef::Owned(Arc::new(d))), false)
	}

	/// Attempts to register many definitions with ID override support.
	pub fn try_register_many_override<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		self.try_register_many_internal(defs.into_iter().map(DefRef::Builtin), true)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefRef<T>>,
	{
		let input_defs: Vec<DefRef<T>> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			let mut seen_identities = rustc_hash::FxHashSet::default();
			for def in cur.by_id.values() {
				seen_identities.insert(def.identity());
			}

			let mut actions = vec![InsertAction::KeptExisting; input_defs.len()];
			let choose_winner = self.make_choose_winner();

			let mutated = {
				let mut store = SnapshotStore {
					snap: &mut next,
					mutated: false,
				};

				for (idx, def) in input_defs.iter().enumerate() {
					if seen_identities.contains(&def.identity()) {
						continue;
					}

					let meta = def.as_entry().meta();

					let id_action = if allow_overrides {
						insert_id_key_runtime(
							&mut store,
							self.label,
							choose_winner,
							meta.id,
							def.clone(),
						)?
					} else {
						insert_typed_key(
							&mut store,
							self.label,
							choose_winner,
							KeyKind::Id,
							meta.id,
							def.clone(),
						)?
					};

					if id_action == InsertAction::KeptExisting {
						continue;
					}

					if id_action == InsertAction::InsertedNew {
						store.snap.id_order.push(Box::from(meta.id));
						store.mutated = true;
					}

					let action = insert_typed_key(
						&mut store,
						self.label,
						choose_winner,
						KeyKind::Name,
						meta.name,
						def.clone(),
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							self.label,
							choose_winner,
							KeyKind::Alias,
							alias,
							def.clone(),
						)?;
					}

					actions[idx] = action;
					seen_identities.insert(def.identity());
				}

				store.mutated
			};

			if !mutated {
				return Ok(actions);
			}

			let next_arc = Arc::new(next);
			let prev = self.snap.compare_and_swap(&cur, next_arc);

			if Arc::ptr_eq(&prev, &cur) {
				return Ok(actions);
			}
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

	/// Returns the number of effective definitions.
	pub fn len(&self) -> usize {
		self.snap.load().id_order.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.snap.load().id_order.is_empty()
	}

	/// Returns all definitions in stable order.
	pub fn all(&self) -> Vec<RegistryRef<T>> {
		let snap = self.snap.load_full();
		snap.id_order
			.iter()
			.filter_map(|id| {
				let def = snap.by_id.get(id)?.clone();
				Some(RegistryRef {
					snap: snap.clone(),
					def,
				})
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
	T: RuntimeEntry,
{
	snap: &'a mut Snapshot<T>,
	mutated: bool,
}

impl<T> KeyStore<T> for SnapshotStore<'_, T>
where
	T: RuntimeEntry,
{
	fn get_id_owner(&self, id: &str) -> Option<DefRef<T>> {
		self.snap.by_id.get(id).cloned()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefRef<T>> {
		self.snap.by_key.get(key).cloned()
	}

	fn set_key_winner(&mut self, key: &str, def: DefRef<T>) {
		self.snap.by_key.insert(Box::from(key), def);
		self.mutated = true;
	}

	fn insert_id(&mut self, id: &str, def: DefRef<T>) -> Option<DefRef<T>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				self.mutated = true;
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(o.get().clone()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefRef<T>) {
		self.snap.by_id.insert(Box::from(id), def);
		self.mutated = true;
	}

	fn evict_def(&mut self, def: DefRef<T>) {
		let len_before = self.snap.by_key.len();
		self.snap.by_key.retain(|_, v| !v.ptr_eq(&def));
		if self.snap.by_key.len() != len_before {
			self.mutated = true;
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
		self.mutated = true;
	}
}
