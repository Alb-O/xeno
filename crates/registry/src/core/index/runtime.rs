//! Runtime registry container with atomic publication.
//!
//! # Role
//!
//! This module provides the thread-safe entrypoint for accessing and updating registry data.
//! It handles the CAS-based extension loop.
//!
//! # Invariants
//!
//! - Concurrent registrations must be linearizable (see `invariants::test_no_lost_updates`).

use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;

use super::snapshot::{RegistryRef, Snapshot, SnapshotGuard};
use super::types::RegistryIndex;
use crate::core::{DenseId, DuplicatePolicy, InternerBuilder, Party, RegistryEntry, Symbol};

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

	/// Looks up a definition by ID, name, or secondary key.
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

	/// Looks up a definition by its untyped key.
	pub fn get_key(&self, key: &crate::core::LookupKey<T, Id>) -> Option<RegistryRef<T, Id>> {
		match key {
			crate::core::LookupKey::Static(s) => self.get(s),
			crate::core::LookupKey::Ref(r) => Some(r.clone()),
		}
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
	pub fn register<In>(&self, def: &'static In) -> Result<RegistryRef<T, Id>, RegisterError<T, Id>>
	where
		In: super::build::BuildEntry<T>,
	{
		self.register_internal(def)
	}

	/// Registers an owned definition at runtime.
	///
	/// # Safety/Invariants
	///
	/// The implementation of [`super::build::BuildEntry::build`] must produce a fully-owned `T`.
	/// It MUST NOT store references to the input definition `In` inside `T`, as the input
	/// definition is dropped after registration (unless it was already static).
	pub fn register_owned<In>(
		&self,
		def: Arc<In>,
	) -> Result<RegistryRef<T, Id>, RegisterError<T, Id>>
	where
		In: super::build::BuildEntry<T>,
	{
		self.register_internal(&*def)
	}

	fn register_internal(
		&self,
		def: &dyn super::build::BuildEntry<T>,
	) -> Result<RegistryRef<T, Id>, RegisterError<T, Id>> {
		use super::lookup::build_lookup;

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

			// 2. Extend key pool
			let mut key_pool = old.key_pool.to_vec();
			let new_entry = def.build(&interner, &mut key_pool);

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
						super::cmp_party(&new_party, &existing_party) == Ordering::Greater
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
				build_lookup(self.label, &new_table, &parties, &key_pool, self.policy);

			// 5. Publish with CAS (clone Arc to return exact snapshot on success)
			let new_snap = Snapshot {
				table: Arc::from(new_table),
				by_key: Arc::new(by_key),
				interner,
				key_pool: Arc::from(key_pool),
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
