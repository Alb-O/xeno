//! Key staging and conflict resolution logic.
//!
//! # Role
//!
//! This module implements the 3-stage key binding model with explicit stage maps:
//! * Stage A (by_id): Canonical IDs - immutable identity
//! * Stage B (by_name): Primary names - display names
//! * Stage C (by_key): Secondary keys - aliases and domain keys
//!
//! # Invariants
//!
//! * Stage B insertions are blocked if the symbol exists in by_id.
//! * Stage C insertions are blocked if the symbol exists in by_id or by_name.
//! * Within-stage conflicts are resolved using Party precedence.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::core::{Collision, CollisionKind, DenseId, KeyKind, Party, Resolution, Symbol};

/// Builds stage B (by_name) and stage C (by_key) maps with collision tracking.
pub(crate) fn build_stage_maps<Out, Id>(
	registry_label: &'static str,
	table: &[Arc<Out>],
	parties: &[Party],
	key_pool: &[Symbol],
	by_id: &FxHashMap<Symbol, Id>,
) -> (FxHashMap<Symbol, Id>, FxHashMap<Symbol, Id>, Vec<Collision>)
where
	Out: super::RuntimeEntry,
	Id: DenseId,
{
	let mut by_name = FxHashMap::default();
	let mut by_key = FxHashMap::default();
	let mut collisions = Vec::new();

	// Stage B: Primary names
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		let name_sym = entry.name();
		let party = parties[idx];

		if by_id.contains_key(&name_sym) {
			if let Some(&canonical_id) = by_id.get(&name_sym) {
				let canonical_party = parties[canonical_id.as_u32() as usize];
				collisions.push(Collision {
					registry: registry_label,
					key: name_sym,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::Canonical,
						incoming_kind: KeyKind::PrimaryName,
						existing: canonical_party,
						incoming: party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
			continue;
		}

		resolve_stage_b(registry_label, name_sym, dense_id, party, parties, &mut by_name, &mut collisions);
	}

	// Stage C: Secondary keys
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		let party = parties[idx];
		let meta = entry.meta();
		let start = meta.keys.start as usize;
		let len = meta.keys.len as usize;

		for &key in &key_pool[start..start + len] {
			if by_id.contains_key(&key) {
				if let Some(&canonical_id) = by_id.get(&key) {
					let canonical_party = parties[canonical_id.as_u32() as usize];
					collisions.push(Collision {
						registry: registry_label,
						key,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::Canonical,
							incoming_kind: KeyKind::SecondaryKey,
							existing: canonical_party,
							incoming: party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
				continue;
			}

			if by_name.contains_key(&key) {
				if let Some(&name_id) = by_name.get(&key) {
					let name_party = parties[name_id.as_u32() as usize];
					collisions.push(Collision {
						registry: registry_label,
						key,
						kind: CollisionKind::KeyConflict {
							existing_kind: KeyKind::PrimaryName,
							incoming_kind: KeyKind::SecondaryKey,
							existing: name_party,
							incoming: party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
				continue;
			}

			resolve_stage_c(registry_label, key, dense_id, party, parties, &mut by_key, &mut collisions);
		}
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_name, by_key, collisions)
}

fn resolve_stage_b<Id>(
	registry_label: &'static str,
	key: Symbol,
	incoming_id: Id,
	incoming_party: Party,
	parties: &[Party],
	by_name: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
) where
	Id: DenseId,
{
	if let Some(&existing_id) = by_name.get(&key) {
		let existing_idx = existing_id.as_u32() as usize;
		let existing_party = parties[existing_idx];

		if super::precedence::party_wins(&incoming_party, &existing_party) {
			by_name.insert(key, incoming_id);
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::PrimaryName,
					incoming_kind: KeyKind::PrimaryName,
					existing: existing_party,
					incoming: incoming_party,
					resolution: Resolution::ReplacedExisting,
				},
			});
		} else {
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::PrimaryName,
					incoming_kind: KeyKind::PrimaryName,
					existing: existing_party,
					incoming: incoming_party,
					resolution: Resolution::KeptExisting,
				},
			});
		}
	} else {
		by_name.insert(key, incoming_id);
	}
}

fn resolve_stage_c<Id>(
	registry_label: &'static str,
	key: Symbol,
	incoming_id: Id,
	incoming_party: Party,
	parties: &[Party],
	by_key: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
) where
	Id: DenseId,
{
	if let Some(&existing_id) = by_key.get(&key) {
		let existing_idx = existing_id.as_u32() as usize;
		let existing_party = parties[existing_idx];

		if super::precedence::party_wins(&incoming_party, &existing_party) {
			by_key.insert(key, incoming_id);
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::SecondaryKey,
					incoming_kind: KeyKind::SecondaryKey,
					existing: existing_party,
					incoming: incoming_party,
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
					incoming: incoming_party,
					resolution: Resolution::KeptExisting,
				},
			});
		}
	} else {
		by_key.insert(key, incoming_id);
	}
}
