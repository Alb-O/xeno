//! Key staging and conflict resolution logic.
//!
//! # Role
//!
//! This module implements the 3-stage key binding model (canonical → name → keys)
//! and records collisions for diagnostics.
//!
//! # Invariants
//!
//! - Canonical IDs must be Displacement-Immune (Stage A cannot be displaced by Stage B/C).
//! - Key conflicts must use the total order tie-breaker ([`crate::core::traits::RegistryEntry::total_order_cmp`]).

use std::cmp::Ordering;
use std::sync::Arc;

use rustc_hash::FxHashMap as HashMap;

use crate::core::{
	Collision, CollisionKind, DenseId, DuplicatePolicy, KeyKind, Party, RegistryEntry, Resolution,
	Symbol,
};

// Make this public so RuntimeRegistry can use it for extension
pub(crate) fn build_lookup<Out, Id>(
	registry_label: &'static str,
	table: &[Arc<Out>],
	parties: &[Party],
	key_pool: &[Symbol],
	_policy: DuplicatePolicy,
) -> (HashMap<Symbol, Id>, Vec<Collision>)
where
	Out: RegistryEntry,
	Id: DenseId,
{
	let mut by_key = HashMap::default();
	let mut collisions = Vec::new();

	#[derive(Copy, Clone, PartialEq, Eq)]
	enum InternalKeyKind {
		Canonical,
		PrimaryName,
		SecondaryKey,
	}
	let mut key_kinds = HashMap::default();

	// Stage A: Canonical IDs
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(idx as u32);
		let sym = entry.meta().id;

		if let Some(_prev_id) = by_key.insert(sym, dense_id) {
			unreachable!(
				"duplicate canonical id reached build_lookup; resolve_id_duplicates should have removed it (registry={}, slot={}, dense_id={}, symbol={})",
				registry_label,
				idx,
				dense_id.as_u32(),
				sym.as_u32()
			);
		}
		key_kinds.insert(sym, InternalKeyKind::Canonical);
	}

	// Stage B: Primary Names
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(idx as u32);
		let name_sym = entry.meta().name;
		let current_party = parties[idx];

		match key_kinds.get(&name_sym) {
			Some(InternalKeyKind::Canonical) => {
				// Canonical ID already occupies this key; record collision but skip binding.
				collisions.push(Collision {
					registry: registry_label,
					key: name_sym,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::Canonical,
						incoming_kind: KeyKind::PrimaryName,
						existing: parties[by_key[&name_sym].as_u32() as usize],
						incoming: current_party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
			Some(InternalKeyKind::PrimaryName) => {
				let existing_id = by_key[&name_sym];
				let existing_idx = existing_id.as_u32() as usize;
				let existing_party = parties[existing_idx];

				if compare_out(entry.as_ref(), table[existing_idx].as_ref()) == Ordering::Greater {
					by_key.insert(name_sym, dense_id);
					collisions.push(Collision {
						registry: registry_label,
						key: name_sym,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::PrimaryName,
							incoming_kind: KeyKind::PrimaryName,
							existing: existing_party,
							incoming: current_party,
							resolution: Resolution::ReplacedExisting,
						},
					});
				} else {
					collisions.push(Collision {
						registry: registry_label,
						key: name_sym,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::PrimaryName,
							incoming_kind: KeyKind::PrimaryName,
							existing: existing_party,
							incoming: current_party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
			}
			None => {
				by_key.insert(name_sym, dense_id);
				key_kinds.insert(name_sym, InternalKeyKind::PrimaryName);
			}
			_ => {}
		}
	}

	// Stage C: Secondary Keys
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(idx as u32);
		let meta = entry.meta();
		let current_party = parties[idx];

		let start = meta.keys.start as usize;
		let len = meta.keys.len as usize;
		debug_assert!(start + len <= key_pool.len());
		let secondary_keys = &key_pool[start..start + len];

		for &key in secondary_keys {
			match key_kinds.get(&key) {
				Some(InternalKeyKind::Canonical) => {
					let existing_id = by_key[&key];
					let existing_idx = existing_id.as_u32() as usize;
					collisions.push(Collision {
						registry: registry_label,
						key,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::Canonical,
							incoming_kind: KeyKind::SecondaryKey,
							existing: parties[existing_idx],
							incoming: current_party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
				Some(InternalKeyKind::PrimaryName) => {
					let existing_id = by_key[&key];
					let existing_idx = existing_id.as_u32() as usize;
					collisions.push(Collision {
						registry: registry_label,
						key,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::PrimaryName,
							incoming_kind: KeyKind::SecondaryKey,
							existing: parties[existing_idx],
							incoming: current_party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
				Some(InternalKeyKind::SecondaryKey) => {
					let existing_id = by_key[&key];
					let existing_idx = existing_id.as_u32() as usize;
					let existing_party = parties[existing_idx];

					if compare_out(entry.as_ref(), table[existing_idx].as_ref())
						== Ordering::Greater
					{
						by_key.insert(key, dense_id);
						collisions.push(Collision {
							registry: registry_label,
							key,
							kind: CollisionKind::KeyConflict {
								existing_kind: KeyKind::SecondaryKey,
								incoming_kind: KeyKind::SecondaryKey,
								existing: existing_party,
								incoming: current_party,
								resolution: Resolution::ReplacedExisting,
							},
						});
					} else {
						collisions.push(Collision {
							registry: registry_label,
							key,
							kind: CollisionKind::KeyConflict {
								existing_kind: KeyKind::SecondaryKey,
								incoming_kind: KeyKind::SecondaryKey,
								existing: existing_party,
								incoming: current_party,
								resolution: Resolution::KeptExisting,
							},
						});
					}
				}
				None => {
					by_key.insert(key, dense_id);
					key_kinds.insert(key, InternalKeyKind::SecondaryKey);
				}
			}
		}
	}

	(by_key, collisions)
}

fn compare_out<T: RegistryEntry>(a: &T, b: &T) -> Ordering {
	// Use the established total order from RegistryEntry.
	// This ensures consistency across all conflict resolution points.
	a.total_order_cmp(b)
}
