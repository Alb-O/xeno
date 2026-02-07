//! Runtime registry implementation with atomic updates.
//!
//! # Purpose
//!
//! The `RuntimeRegistry` manages a collection of definitions that can be extended at runtime.
//! It uses a copy-on-write snapshot strategy to ensure lock-free concurrent reads while allowing
//! atomic updates for new definitions (e.g., from plugins).
//!
//! # Mental Model
//!
//! - **Snapshot:** An immutable, consistent view of the registry state (tables, lookups, interners).
//! - **RegistryRef:** A pinned reference to a definition within a specific snapshot. It keeps the
//!   snapshot alive even if the global registry is updated.
//! - **Atomic Swap:** Updates are performed by building a *new* snapshot and atomically swapping
//!   the pointer. Readers holding old snapshots continue to see the old state.
//!
//! # Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`RuntimeRegistry`] | The main entry point; holds the atomic pointer to the current snapshot. |
//! | [`Snapshot`] | A complete, immutable state of the registry (tables, hashmaps, interner). |
//! | [`RegistryRef`] | A handle to a definition, carrying a strong reference to its source snapshot. |
//! | [`SnapshotGuard`] | A lightweight guard for efficient iteration without per-item Arc clones. |
//!
//! # Invariants
//!
//! - Must have unambiguous ID lookup (one winner per ID).
//!   - Enforced in: [`crate::core::index::build::resolve_id_duplicates`] (build time), [`RuntimeRegistry::register`] (runtime).
//!   - Tested by: [`crate::core::index::invariants::test_unambiguous_id_lookup`]
//!   - Failure symptom: Panics or inconsistent lookups.
//!
//! - Must maintain deterministic iteration order.
//!   - Enforced in: [`crate::core::index::build::RegistryBuilder::build`] (sort by ID).
//!   - Tested by: [`crate::core::index::invariants::test_deterministic_iteration`]
//!   - Failure symptom: Iterator order changes unpredictably.
//!
//! - Must keep owned definitions alive while reachable.
//!   - Enforced in: [`RegistryRef`] (holds `Arc<Snapshot>`).
//!   - Tested by: [`crate::core::index::invariants::test_snapshot_liveness_across_swap`]
//!   - Failure symptom: Use-after-free in `RegistryRef` deref.
//!
//! - Must provide linearizable writes without lost updates.
//!   - Enforced in: [`RuntimeRegistry::register`] (CAS loop).
//!   - Tested by: [`crate::core::index::invariants::test_no_lost_updates`]
//!   - Failure symptom: Concurrent registrations silently dropped.
//!
//! # Data Flow
//!
//! 1. **Read:** `registry.get()` loads the current snapshot `Arc`. It performs a lookup and returns
//!    a `RegistryRef` wrapping that same `Arc`.
//! 2. **Write:** `registry.register()` uses a CAS loop:
//!    - Load current snapshot
//!    - Build extended snapshot (new interner, new table with Arc-wrapped entries)
//!    - CAS swap; if failed, retry with updated snapshot
//!
//! # Concurrency
//!
//! - **Reads:** Wait-free (atomic load).
//! - **Writes:** Lock-free with linearizability (CAS retry loop). No lost updates under contention.

use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap;

use super::types::RegistryIndex;
use crate::core::{
	Collision, DenseId, DuplicatePolicy, FrozenInterner, InternerBuilder, Party, RegistryEntry,
	Symbol,
};

/// Error type for registration failures.
pub enum RegisterError<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	/// Registration was rejected due to duplicate policy (existing higher priority).
	Rejected {
		/// Reference to the existing winning definition.
		existing: RegistryRef<T, Id>,
		/// ID string of the incoming (rejected) definition.
		incoming_id: String,
		/// The policy that caused the rejection.
		policy: DuplicatePolicy,
	},
}

impl<T, Id: DenseId> std::fmt::Debug for RegisterError<T, Id>
where
	T: RuntimeEntry,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			RegisterError::Rejected {
				existing,
				incoming_id,
				policy,
			} => f
				.debug_struct("Rejected")
				.field("existing_id", &existing.id_str())
				.field("incoming_id", incoming_id)
				.field("policy", policy)
				.finish(),
		}
	}
}

impl<T, Id: DenseId> std::fmt::Display for RegisterError<T, Id>
where
	T: RuntimeEntry,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			RegisterError::Rejected {
				existing,
				incoming_id,
				policy,
			} => {
				write!(
					f,
					"Registration rejected: '{}' lost to existing '{}' under policy {:?}",
					incoming_id,
					existing.id_str(),
					policy
				)
			}
		}
	}
}

impl<T, Id: DenseId> std::error::Error for RegisterError<T, Id> where T: RuntimeEntry {}

/// Marker trait for types that can be stored in a runtime registry.
pub trait RuntimeEntry: RegistryEntry + Send + Sync + 'static {}
impl<T> RuntimeEntry for T where T: RegistryEntry + Send + Sync + 'static {}

/// Snapshot-pinning guard that provides `&T` access to a registry definition.
pub struct RegistryRef<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub(crate) snap: Arc<Snapshot<T, Id>>,
	pub(crate) id: Id,
}

impl<T, Id: DenseId> Clone for RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			id: self.id,
		}
	}
}

impl<T, Id: DenseId> std::fmt::Debug for RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RegistryRef")
			.field("id", &self.id)
			.field("name", &self.name_str())
			.finish()
	}
}

impl<T, Id: DenseId> RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	/// Returns the dense ID for this definition.
	pub fn dense_id(&self) -> Id {
		self.id
	}

	/// Resolves a symbol to its string representation using this ref's snapshot interner.
	pub fn resolve(&self, sym: crate::core::Symbol) -> &str {
		self.snap.interner.resolve(sym)
	}

	/// Returns the interned name as a string.
	pub fn name_str(&self) -> &str {
		self.resolve(self.name())
	}

	/// Returns the interned id as a string.
	pub fn id_str(&self) -> &str {
		self.resolve(self.id())
	}

	/// Returns the interned description as a string.
	pub fn description_str(&self) -> &str {
		self.resolve(self.description())
	}

	/// Returns an iterator over resolved alias strings.
	pub fn aliases_resolved(&self) -> Vec<&str> {
		let meta = self.meta();
		let start = meta.aliases.start as usize;
		let end = start + meta.aliases.len as usize;
		self.snap.alias_pool[start..end]
			.iter()
			.map(|&sym| self.snap.interner.resolve(sym))
			.collect()
	}
}

impl<T, Id: DenseId> std::ops::Deref for RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	type Target = T;

	fn deref(&self) -> &T {
		&self.snap.table[self.id.as_u32() as usize]
	}
}

/// Single source of truth for registry lookups.
pub struct Snapshot<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub table: Arc<[Arc<T>]>,
	pub by_key: Arc<FxHashMap<Symbol, Id>>,
	pub interner: FrozenInterner,
	pub alias_pool: Arc<[Symbol]>,
	pub collisions: Arc<[Collision]>,
}

impl<T, Id: DenseId> Clone for Snapshot<T, Id>
where
	T: RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			table: self.table.clone(),
			by_key: self.by_key.clone(),
			interner: self.interner.clone(),
			alias_pool: self.alias_pool.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T, Id: DenseId> Snapshot<T, Id>
where
	T: RuntimeEntry,
{
	/// Creates a new snapshot from a builtin index.
	fn from_builtins(b: &RegistryIndex<T, Id>) -> Self {
		Self {
			table: b.table.clone(),
			by_key: b.by_key.clone(),
			interner: b.interner.clone(),
			alias_pool: b.alias_pool.clone(),
			collisions: b.collisions.clone(),
		}
	}
}

/// Lightweight guard for efficient iteration without per-item Arc clones.
pub struct SnapshotGuard<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	snap: Arc<Snapshot<T, Id>>,
}

impl<T, Id: DenseId> SnapshotGuard<T, Id>
where
	T: RuntimeEntry,
{
	/// Returns an iterator over all entries in the snapshot.
	pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
		self.snap.table.iter().map(|arc| arc.as_ref())
	}

	/// Returns an iterator over (Id, &T) pairs.
	pub fn iter_items(&self) -> impl Iterator<Item = (Id, &T)> + '_ {
		self.snap
			.table
			.iter()
			.enumerate()
			.map(|(idx, arc)| (Id::from_u32(idx as u32), arc.as_ref()))
	}

	/// Returns the number of entries.
	pub fn len(&self) -> usize {
		self.snap.table.len()
	}

	/// Returns true if empty.
	pub fn is_empty(&self) -> bool {
		self.snap.table.is_empty()
	}
}

/// Registry wrapper for runtime-extensible registries.
pub struct RuntimeRegistry<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub(super) label: &'static str,
	#[allow(dead_code)]
	pub(super) builtins: RegistryIndex<T, Id>,
	pub(super) snap: ArcSwap<Snapshot<T, Id>>,
	pub(super) policy: DuplicatePolicy,
}

impl<T, Id: DenseId> RuntimeRegistry<T, Id>
where
	T: RuntimeEntry,
{
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T, Id>) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self {
			label,
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy: DuplicatePolicy::default(), // Runtime defaults to ByPriority
		}
	}

	/// Creates a new runtime registry with the given builtins and duplicate policy.
	pub fn with_policy(
		label: &'static str,
		builtins: RegistryIndex<T, Id>,
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
	pub fn get(&self, key: &str) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let sym = snap.interner.get(key)?;
		let id = *snap.by_key.get(&sym)?;
		Some(RegistryRef { snap, id })
	}

	/// Looks up a definition by its interned symbol.
	#[inline]
	pub fn get_sym(&self, sym: Symbol) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let id = *snap.by_key.get(&sym)?;
		Some(RegistryRef { snap, id })
	}

	/// Returns all effective definitions.
	pub fn all(&self) -> Vec<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let mut refs = Vec::with_capacity(snap.table.len());
		for i in 0..snap.table.len() {
			refs.push(RegistryRef {
				snap: snap.clone(),
				id: Id::from_u32(i as u32),
			});
		}
		refs
	}

	/// Returns a snapshot guard for efficient iteration.
	pub fn snapshot_guard(&self) -> SnapshotGuard<T, Id> {
		SnapshotGuard {
			snap: self.snap.load_full(),
		}
	}

	/// Looks up a definition by its dense ID.
	#[inline]
	pub fn get_by_id(&self, id: Id) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		if (id.as_u32() as usize) < snap.table.len() {
			Some(RegistryRef { snap, id })
		} else {
			None
		}
	}

	/// Returns a snapshot guard for direct interner access.
	pub fn snapshot(&self) -> Arc<Snapshot<T, Id>> {
		self.snap.load_full()
	}

	/// Registers a new definition at runtime with linearizable semantics.
	///
	/// Uses a CAS loop to ensure no lost updates under contention.
	/// Returns `Ok(RegistryRef)` if successfully registered (won or replaced).
	/// Returns `Err(RegisterError::Rejected)` if rejected due to policy.
	pub fn register<In>(&self, def: &'static In) -> Result<RegistryRef<T, Id>, RegisterError<T, Id>>
	where
		In: super::BuildEntry<T>,
	{
		use crate::core::index::build::build_lookup;

		loop {
			let old = self.snap.load_full();

			// 1. Extend interner
			let mut ib = InternerBuilder::from_frozen(&old.interner);
			let mut tmp_strings = Vec::new();
			def.collect_strings(&mut tmp_strings);
			tmp_strings.sort_unstable();
			tmp_strings.dedup();

			for s in tmp_strings {
				ib.intern(s);
			}
			let interner = ib.freeze();

			// 2. Extend alias pool
			let mut alias_pool = old.alias_pool.to_vec();
			let new_entry = def.build(&interner, &mut alias_pool);

			// 3. Resolve canonical ID collision with existing
			let id_sym = new_entry.meta().id;

			let mut new_table: Vec<Arc<T>> = old.table.to_vec();
			let mut parties: Vec<Party> = Vec::with_capacity(new_table.len() + 1);

			// Reconstruct parties from existing table
			for (i, entry) in old.table.iter().enumerate() {
				parties.push(Party {
					def_id: entry.meta().id,
					source: entry.meta().source,
					priority: entry.meta().priority,
					ordinal: i as u32,
				});
			}

			// Check for CANONICAL ID collision
			let existing_idx = old.table.iter().position(|e| e.meta().id == id_sym);
			let mut replaced_idx = None;
			let mut new_idx = new_table.len();

			if let Some(idx) = existing_idx {
				// Collision on canonical ID
				let existing_party = parties[idx];

				// Compare new vs existing using same logic as build-time
				let new_party = Party {
					def_id: id_sym,
					source: new_entry.meta().source,
					priority: new_entry.meta().priority,
					ordinal: new_table.len() as u32,
				};

				// Policy check: higher priority wins; at equal priority, higher
				// source rank wins (Runtime > Crate > Builtin), so runtime
				// extensions naturally override builtins without needing an
				// explicit priority bump.
				let is_better = match self.policy {
					DuplicatePolicy::FirstWins => false,
					DuplicatePolicy::LastWins => true,
					DuplicatePolicy::ByPriority => {
						let ord =
							new_party
								.priority
								.cmp(&existing_party.priority)
								.then_with(|| {
									new_party.source.rank().cmp(&existing_party.source.rank())
								});
						ord == Ordering::Greater
					}
					DuplicatePolicy::Panic => {
						panic!(
							"Duplicate runtime registry key: {}",
							interner.resolve(new_entry.meta().id)
						)
					}
				};

				if is_better {
					// Replace existing
					new_table[idx] = Arc::new(new_entry);
					parties[idx] = new_party;
					replaced_idx = Some(idx);
				} else {
					// New entry lost - return error with existing ref
					let existing_id = Id::from_u32(idx as u32);
					return Err(RegisterError::Rejected {
						existing: RegistryRef {
							snap: old,
							id: existing_id,
						},
						incoming_id: def.meta_ref().id.to_string(),
						policy: self.policy,
					});
				}
			} else {
				// New unique ID
				let ordinal = new_table.len() as u32;
				parties.push(Party {
					def_id: id_sym,
					source: new_entry.meta().source,
					priority: new_entry.meta().priority,
					ordinal,
				});
				new_table.push(Arc::new(new_entry));
				new_idx = new_table.len() - 1;
			}

			// 4. Rebuild lookup and collisions
			let (by_key, key_collisions) =
				build_lookup(self.label, &new_table, &parties, &alias_pool, self.policy);

			// 5. Publish with CAS (clone Arc to return exact snapshot on success)
			let new_snap = Snapshot {
				table: Arc::from(new_table),
				by_key: Arc::new(by_key),
				interner,
				alias_pool: Arc::from(alias_pool),
				collisions: Arc::from(key_collisions),
			};
			let new_arc = Arc::new(new_snap);

			let prev = self.snap.compare_and_swap(&old, new_arc.clone());

			if Arc::ptr_eq(&prev, &old) {
				// CAS succeeded - return ref pinned to exact snapshot we installed
				let result_id = replaced_idx
					.map(|i| Id::from_u32(i as u32))
					.unwrap_or_else(|| Id::from_u32(new_idx as u32));
				return Ok(RegistryRef {
					snap: new_arc,
					id: result_id,
				});
			}
			// CAS failed, retry with updated snapshot
		}
	}

	/// Returns an iterator over all definitions as `RegistryRef`s.
	pub fn iter(&self) -> Vec<RegistryRef<T, Id>> {
		self.all()
	}

	/// Returns the number of effective definitions.
	pub fn len(&self) -> usize {
		self.snap.load().table.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}
