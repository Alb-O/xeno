//! Key staging and conflict resolution logic.
//!
//! # Role
//!
//! This module implements the 3-stage key binding model (canonical → name → aliases)
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
	alias_pool: &[Symbol],
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
		Name,
		Alias,
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

	// Stage B: Names
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(idx as u32);
		let name_sym = entry.meta().name;
		let current_party = parties[idx];

		match key_kinds.get(&name_sym) {
			Some(InternalKeyKind::Canonical) => {
				// Canonical ID already occupies this key; skip silently.
			}
			Some(InternalKeyKind::Name) => {
				let existing_id = by_key[&name_sym];
				let existing_idx = existing_id.as_u32() as usize;
				let existing_party = parties[existing_idx];

				if compare_out(entry.as_ref(), table[existing_idx].as_ref()) == Ordering::Greater {
					by_key.insert(name_sym, dense_id);
					collisions.push(Collision {
						registry: registry_label,
						key: name_sym,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::Alias, // Names are treated as aliases for conflict purposes
							incoming_kind: KeyKind::Alias,
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
							existing_kind: KeyKind::Alias,
							incoming_kind: KeyKind::Alias,
							existing: existing_party,
							incoming: current_party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
			}
			None => {
				by_key.insert(name_sym, dense_id);
				key_kinds.insert(name_sym, InternalKeyKind::Name);
			}
			_ => {}
		}
	}

	// Stage C: Aliases
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(idx as u32);
		let meta = entry.meta();
		let current_party = parties[idx];

		let start = meta.aliases.start as usize;
		let len = meta.aliases.len as usize;
		debug_assert!(start + len <= alias_pool.len());
		let aliases = &alias_pool[start..start + len];

		for &alias in aliases {
			match key_kinds.get(&alias) {
				Some(InternalKeyKind::Canonical | InternalKeyKind::Name) => {
					let existing_id = by_key[&alias];
					let existing_idx = existing_id.as_u32() as usize;
					collisions.push(Collision {
						registry: registry_label,
						key: alias,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::Canonical, // Simplification: assume it's canonical/name
							incoming_kind: KeyKind::Alias,
							existing: parties[existing_idx],
							incoming: current_party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
				Some(InternalKeyKind::Alias) => {
					let existing_id = by_key[&alias];
					let existing_idx = existing_id.as_u32() as usize;
					let existing_party = parties[existing_idx];

					if compare_out(entry.as_ref(), table[existing_idx].as_ref())
						== Ordering::Greater
					{
						by_key.insert(alias, dense_id);
						collisions.push(Collision {
							registry: registry_label,
							key: alias,
							kind: CollisionKind::KeyConflict {
								existing_kind: KeyKind::Alias,
								incoming_kind: KeyKind::Alias,
								existing: existing_party,
								incoming: current_party,
								resolution: Resolution::ReplacedExisting,
							},
						});
					} else {
						collisions.push(Collision {
							registry: registry_label,
							key: alias,
							kind: CollisionKind::KeyConflict {
								existing_kind: KeyKind::Alias,
								incoming_kind: KeyKind::Alias,
								existing: existing_party,
								incoming: current_party,
								resolution: Resolution::KeptExisting,
							},
						});
					}
				}
				None => {
					by_key.insert(alias, dense_id);
					key_kinds.insert(alias, InternalKeyKind::Alias);
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
