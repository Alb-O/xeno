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
		/// Metadata for the existing winner.
		existing_party: Party,
		/// Metadata for the incoming loser.
		incoming_party: Party,
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
				existing_party,
				incoming_party,
			} => f
				.debug_struct("Rejected")
				.field("existing_id", &existing.id_str())
				.field("incoming_id", incoming_id)
				.field("policy", policy)
				.field("existing_party", existing_party)
				.field("incoming_party", incoming_party)
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
				existing_party,
				incoming_party,
			} => {
				write!(
					f,
					"Registration rejected: '{}' ({:?}) lost to existing '{}' ({:?}) under policy {:?}",
					incoming_id,
					incoming_party,
					existing.id_str(),
					existing_party,
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
	///
	/// Uses 3-stage fallback: canonical ID → primary name → secondary keys.
	#[inline]
	pub fn get(&self, key: &str) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let sym = snap.interner.get(key)?;
		self.get_sym_with_snap(snap, sym)
	}

	/// Looks up a definition by its interned symbol.
	///
	/// Uses 3-stage fallback: canonical ID → primary name → secondary keys.
	#[inline]
	pub fn get_sym(&self, sym: Symbol) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		self.get_sym_with_snap(snap, sym)
	}

	#[inline]
	fn get_sym_with_snap(
		&self,
		snap: Arc<Snapshot<T, Id>>,
		sym: Symbol,
	) -> Option<RegistryRef<T, Id>> {
		let id = snap
			.by_id
			.get(&sym)
			.or_else(|| snap.by_name.get(&sym))
			.or_else(|| snap.by_key.get(&sym))
			.copied()?;
		Some(RegistryRef { snap, id })
	}

	/// Looks up a definition by its untyped key.
	pub fn get_key(&self, key: &crate::core::LookupKey<T, Id>) -> Option<RegistryRef<T, Id>> {
		match key {
			crate::core::LookupKey::Static(s) => self.get(s),
			crate::core::LookupKey::Ref(r) => Some(r.clone()),
		}
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
		loop {
			let old = self.snap.load_full();

			// 1. Extend interner
			let mut ib = InternerBuilder::from_frozen(&old.interner);
			let mut tmp_strings = Vec::new();
			def.collect_strings_all(&mut tmp_strings);
			tmp_strings.sort_unstable();
			tmp_strings.dedup();

			for s in tmp_strings {
				ib.intern(s);
			}
			let interner = ib.freeze();

			// 2. Extend key pool
			let mut key_pool = old.key_pool.to_vec();
			let mut runtime_ctx = super::build::RuntimeBuildCtx {
				interner: &interner,
			};

			#[cfg(any(debug_assertions, feature = "registry-contracts"))]
			let new_entry = {
				let mut sink = Vec::new();
				def.collect_strings_all(&mut sink);
				let collected = sink.into_iter().collect();
				let mut ctx = super::build::DebugBuildCtx {
					inner: &mut runtime_ctx,
					collected,
					used: std::collections::HashSet::default(),
				};
				def.build(&mut ctx, &mut key_pool)
			};

			#[cfg(not(any(debug_assertions, feature = "registry-contracts")))]
			let new_entry = def.build(&mut runtime_ctx, &mut key_pool);

			// 3. Resolve canonical ID collision with existing
			let id_sym = new_entry.meta().id;

			let mut new_table: Vec<Arc<T>> = old.table.to_vec();
			let mut parties: Vec<Party> = old.parties.to_vec();
			let mut new_by_id = (*old.by_id).clone();

			// Check for CANONICAL ID collision using O(1) by_id lookup
			let existing_id = old.by_id.get(&id_sym).copied();
			let existing_idx = existing_id.map(|id| id.as_u32() as usize);

			let mut replaced_info = None;
			let mut new_idx = new_table.len();

			// Get monotonic ordinal for this registration attempt
			let new_ordinal = old.next_ordinal;

			if let Some(idx) = existing_idx {
				// Collision on canonical ID
				let existing_party = parties[idx];

				// Compare new vs existing using same logic as build-time
				let new_party = Party {
					def_id: id_sym,
					source: new_entry.meta().source,
					priority: new_entry.meta().priority,
					ordinal: new_ordinal,
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
					replaced_info = Some((idx, existing_party, new_party));
				} else {
					// New entry lost - return error with existing ref
					let existing_id_id = Id::from_u32(super::u32_index(idx, self.label));
					return Err(RegisterError::Rejected {
						existing: RegistryRef {
							snap: old,
							id: existing_id_id,
						},
						incoming_id: def.meta_ref().id.to_string(),
						policy: self.policy,
						existing_party,
						incoming_party: new_party,
					});
				}
			} else {
				// New unique ID
				parties.push(Party {
					def_id: id_sym,
					source: new_entry.meta().source,
					priority: new_entry.meta().priority,
					ordinal: new_ordinal,
				});
				new_table.push(Arc::new(new_entry));
				new_idx = new_table.len() - 1;
				new_by_id.insert(id_sym, Id::from_u32(super::u32_index(new_idx, self.label)));
			}

			// 4. Update stage maps (by_name, by_key) and collisions incrementally
			let (by_name, by_key, mut key_collisions) = if let Some((idx, _, _)) = replaced_info {
				super::lookup::update_stage_maps_replace(
					self.label, &new_table, &parties, &key_pool, idx, &old, &new_by_id,
				)
			} else {
				// Incremental append
				super::lookup::update_stage_maps_append(
					self.label,
					&new_table,
					&parties,
					&key_pool,
					new_idx,
					&new_by_id,
					&old.by_name,
					&old.by_key,
					&old.collisions,
				)
			};

			// Record DuplicateId collision for runtime replacements
			if let Some((_, existing_party, new_party)) = replaced_info {
				use crate::core::index::collision::{Collision, CollisionKind};

				// Remove any existing DuplicateId record for this canonical ID to keep it bounded
				key_collisions.retain(|c| {
					!matches!(c.kind, CollisionKind::DuplicateId { .. }) || c.key != id_sym
				});

				key_collisions.push(Collision {
					registry: self.label,
					key: id_sym,
					kind: CollisionKind::DuplicateId {
						winner: new_party,
						loser: existing_party,
						policy: self.policy,
					},
				});
				key_collisions.sort_by(Collision::stable_cmp);
			}

			// 5. Publish with CAS (clone Arc to return exact snapshot on success)
			let new_snap = Snapshot {
				table: Arc::from(new_table),
				by_id: Arc::new(new_by_id),
				by_name: Arc::new(by_name),
				by_key: Arc::new(by_key),
				interner,
				key_pool: Arc::from(key_pool),
				collisions: Arc::from(key_collisions),
				parties: Arc::from(parties),
				next_ordinal: new_ordinal.saturating_add(1),
			};
			let new_arc = Arc::new(new_snap);

			let prev = self.snap.compare_and_swap(&old, new_arc.clone());

			if Arc::ptr_eq(&prev, &old) {
				// CAS succeeded - return ref pinned to exact snapshot we installed
				let result_id = replaced_info
					.map(|(i, _, _)| Id::from_u32(super::u32_index(i, self.label)))
					.unwrap_or_else(|| Id::from_u32(super::u32_index(new_idx, self.label)));
				return Ok(RegistryRef {
					snap: new_arc,
					id: result_id,
				});
			}
			// CAS failed, retry with updated snapshot
		}
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
