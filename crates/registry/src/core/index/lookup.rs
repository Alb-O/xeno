//! Key staging and conflict resolution logic.
//!
//! # Role
//!
//! This module implements the 3-stage key binding model with explicit stage maps:
//! - Stage A (by_id): Canonical IDs - immutable identity
//! - Stage B (by_name): Primary names - display names
//! - Stage C (by_key): Secondary keys - aliases and domain keys
//!
//! # Invariants
//!
//! - Stage B insertions are blocked if the symbol exists in by_id.
//! - Stage C insertions are blocked if the symbol exists in by_id or by_name.
//! - Within-stage conflicts are resolved using total order comparison.

use std::cmp::Ordering;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::core::{
	Collision, CollisionKind, DenseId, KeyKind, Party, RegistryEntry, Resolution, Snapshot, Symbol,
};

/// Collects all lookup symbols for an entry that may require collision recalculation.
pub(crate) fn collect_symbols_for_collision_recalc<T: RegistryEntry>(
	entry: &T,
	key_pool: &[Symbol],
	sink: &mut std::collections::HashSet<Symbol>,
) {
	sink.insert(entry.id());
	sink.insert(entry.name());
	let meta = entry.meta();
	let start = meta.keys.start as usize;
	let len = meta.keys.len as usize;
	for &key in &key_pool[start..start + len] {
		sink.insert(key);
	}
}

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

	// Stage B: Primary Names
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		let name_sym = entry.name();
		let party = parties[idx];

		// Check for Stage A block
		if by_id.contains_key(&name_sym) {
			// Record collision: Canonical blocks PrimaryName
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

		// Within-stage conflict resolution
		resolve_stage_b(
			registry_label,
			name_sym,
			dense_id,
			party,
			table,
			parties,
			&mut by_name,
			&mut collisions,
		);
	}

	// Stage C: Secondary Keys
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		let party = parties[idx];
		let meta = entry.meta();
		let start = meta.keys.start as usize;
		let len = meta.keys.len as usize;

		for &key in &key_pool[start..start + len] {
			// Check for Stage A or B block
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

			// Within-stage conflict resolution
			resolve_stage_c(
				registry_label,
				key,
				dense_id,
				party,
				table,
				parties,
				&mut by_key,
				&mut collisions,
			);
		}
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_name, by_key, collisions)
}

fn resolve_stage_b<Out, Id>(
	registry_label: &'static str,
	key: Symbol,
	incoming_id: Id,
	incoming_party: Party,
	table: &[Arc<Out>],
	parties: &[Party],
	by_name: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
) where
	Out: RegistryEntry,
	Id: DenseId,
{
	if let Some(&existing_id) = by_name.get(&key) {
		let existing_idx = existing_id.as_u32() as usize;
		let existing_party = parties[existing_idx];

		// Same stage: compare entries
		if table[incoming_id.as_u32() as usize].total_order_cmp(table[existing_idx].as_ref())
			== Ordering::Greater
		{
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

fn resolve_stage_c<Out, Id>(
	registry_label: &'static str,
	key: Symbol,
	incoming_id: Id,
	incoming_party: Party,
	table: &[Arc<Out>],
	parties: &[Party],
	by_key: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
) where
	Out: RegistryEntry,
	Id: DenseId,
{
	if let Some(&existing_id) = by_key.get(&key) {
		let existing_idx = existing_id.as_u32() as usize;
		let existing_party = parties[existing_idx];

		// Same stage: compare entries
		if table[incoming_id.as_u32() as usize].total_order_cmp(table[existing_idx].as_ref())
			== Ordering::Greater
		{
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

/// Incremental update for append operations.
pub(crate) fn update_stage_maps_append<T, Id>(
	registry_label: &'static str,
	table: &[Arc<T>],
	parties: &[Party],
	key_pool: &[Symbol],
	new_idx: usize,
	by_id: &FxHashMap<Symbol, Id>,
	old_by_name: &FxHashMap<Symbol, Id>,
	old_by_key: &FxHashMap<Symbol, Id>,
	old_collisions: &[Collision],
) -> (FxHashMap<Symbol, Id>, FxHashMap<Symbol, Id>, Vec<Collision>)
where
	T: super::RuntimeEntry,
	Id: DenseId,
{
	let mut by_name = old_by_name.clone();
	let mut by_key = old_by_key.clone();
	let mut collisions = old_collisions.to_vec();

	let entry = &table[new_idx];
	let dense_id = Id::from_u32(super::u32_index(new_idx, registry_label));
	let party = parties[new_idx];

	// Stage B: Primary Name
	let name_sym = entry.name();
	if let Some(&canonical_id) = by_id.get(&name_sym) {
		// Record collision: Canonical blocks PrimaryName
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
	} else {
		resolve_stage_b(
			registry_label,
			name_sym,
			dense_id,
			party,
			table,
			parties,
			&mut by_name,
			&mut collisions,
		);
	}

	// Stage C: Secondary Keys
	let meta = entry.meta();
	let start = meta.keys.start as usize;
	let len = meta.keys.len as usize;
	for &key in &key_pool[start..start + len] {
		if let Some(&canonical_id) = by_id.get(&key) {
			// Record collision: Canonical blocks SecondaryKey
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
			continue;
		}

		if let Some(&name_id) = by_name.get(&key) {
			// Record collision: PrimaryName blocks SecondaryKey
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
			continue;
		}

		resolve_stage_c(
			registry_label,
			key,
			dense_id,
			party,
			table,
			parties,
			&mut by_key,
			&mut collisions,
		);
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_name, by_key, collisions)
}

/// Incremental update for replace operations.
pub(crate) fn update_stage_maps_replace<T, Id>(
	registry_label: &'static str,
	table: &[Arc<T>],
	parties: &[Party],
	key_pool: &[Symbol],
	replaced_idx: usize,
	old_snap: &Snapshot<T, Id>,
	new_by_id: &FxHashMap<Symbol, Id>,
) -> (FxHashMap<Symbol, Id>, FxHashMap<Symbol, Id>, Vec<Collision>)
where
	T: super::RuntimeEntry,
	Id: DenseId,
{
	let mut by_name = (*old_snap.by_name).clone();
	let mut by_key = (*old_snap.by_key).clone();

	// Collect all affected keys from old and new entry
	let mut affected_keys = std::collections::HashSet::default();
	collect_symbols_for_collision_recalc(
		old_snap.table[replaced_idx].as_ref(),
		old_snap.key_pool.as_ref(),
		&mut affected_keys,
	);
	collect_symbols_for_collision_recalc(
		table[replaced_idx].as_ref(),
		key_pool,
		&mut affected_keys,
	);

	// Filter out collisions for affected keys (will be recomputed)
	let mut collisions: Vec<Collision> = old_snap
		.collisions
		.iter()
		.filter(|c| !affected_keys.contains(&c.key))
		.cloned()
		.collect();

	// Recalculate winners for affected keys
	// First Stage B, then Stage C (because Stage C depends on Stage B)
	for &key in &affected_keys {
		recalculate_stage_b_winner(
			registry_label,
			key,
			&mut by_name,
			&mut collisions,
			table,
			parties,
			new_by_id,
		);
	}

	for &key in &affected_keys {
		recalculate_stage_c_winner(
			registry_label,
			key,
			&mut by_key,
			&mut collisions,
			table,
			parties,
			key_pool,
			new_by_id,
			&by_name,
		);
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_name, by_key, collisions)
}

fn recalculate_stage_b_winner<T, Id>(
	registry_label: &'static str,
	key: Symbol,
	by_name: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
	table: &[Arc<T>],
	parties: &[Party],
	by_id: &FxHashMap<Symbol, Id>,
) where
	T: RegistryEntry,
	Id: DenseId,
{
	// Check Stage A block first - record collisions for all attempted Stage B binds
	if let Some(&canon_id) = by_id.get(&key) {
		let canon_party = parties[canon_id.as_u32() as usize];

		// Record collision for every entry whose name matches this blocked key
		for (i, entry) in table.iter().enumerate() {
			if entry.name() == key {
				let incoming_party = parties[i];
				collisions.push(Collision {
					registry: registry_label,
					key,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::Canonical,
						incoming_kind: KeyKind::PrimaryName,
						existing: canon_party,
						incoming: incoming_party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
		}

		by_name.remove(&key);
		return;
	}

	// Find all candidates for this key in Stage B
	let mut candidates: Vec<(Id, Party)> = Vec::new();
	for (i, entry) in table.iter().enumerate() {
		if entry.name() == key {
			let dense_id = Id::from_u32(super::u32_index(i, registry_label));
			candidates.push((dense_id, parties[i]));
		}
	}

	if candidates.is_empty() {
		by_name.remove(&key);
		return;
	}

	// Find winner using total order
	let (mut winner_id, mut winner_party) = candidates[0];
	for (challenger_id, challenger_party) in candidates.into_iter().skip(1) {
		if table[challenger_id.as_u32() as usize]
			.total_order_cmp(table[winner_id.as_u32() as usize].as_ref())
			== Ordering::Greater
		{
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::PrimaryName,
					incoming_kind: KeyKind::PrimaryName,
					existing: winner_party,
					incoming: challenger_party,
					resolution: Resolution::ReplacedExisting,
				},
			});
			winner_id = challenger_id;
			winner_party = challenger_party;
		} else {
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::PrimaryName,
					incoming_kind: KeyKind::PrimaryName,
					existing: winner_party,
					incoming: challenger_party,
					resolution: Resolution::KeptExisting,
				},
			});
		}
	}

	by_name.insert(key, winner_id);
}

fn entry_has_secondary_key<T: RegistryEntry>(
	entry: &Arc<T>,
	key_pool: &[Symbol],
	key: Symbol,
) -> bool {
	let meta = entry.meta();
	let start = meta.keys.start as usize;
	let len = meta.keys.len as usize;
	key_pool[start..start + len].contains(&key)
}

fn recalculate_stage_c_winner<T, Id>(
	registry_label: &'static str,
	key: Symbol,
	by_key: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
	table: &[Arc<T>],
	parties: &[Party],
	key_pool: &[Symbol],
	by_id: &FxHashMap<Symbol, Id>,
	by_name: &FxHashMap<Symbol, Id>,
) where
	T: RegistryEntry,
	Id: DenseId,
{
	// Check Stage A block - canonical ID blocks secondary keys
	if let Some(&canon_id) = by_id.get(&key) {
		let canon_party = parties[canon_id.as_u32() as usize];

		// Record collision for every entry that has this key as secondary
		for (i, entry) in table.iter().enumerate() {
			if entry_has_secondary_key(entry, key_pool, key) {
				let incoming_party = parties[i];
				collisions.push(Collision {
					registry: registry_label,
					key,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::Canonical,
						incoming_kind: KeyKind::SecondaryKey,
						existing: canon_party,
						incoming: incoming_party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
		}

		by_key.remove(&key);
		return;
	}

	// Check Stage B block - primary name blocks secondary keys
	if let Some(&name_id) = by_name.get(&key) {
		let name_party = parties[name_id.as_u32() as usize];

		// Record collision for every entry that has this key as secondary
		for (i, entry) in table.iter().enumerate() {
			if entry_has_secondary_key(entry, key_pool, key) {
				let incoming_party = parties[i];
				collisions.push(Collision {
					registry: registry_label,
					key,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::PrimaryName,
						incoming_kind: KeyKind::SecondaryKey,
						existing: name_party,
						incoming: incoming_party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
		}

		by_key.remove(&key);
		return;
	}

	// Find all candidates for this key in Stage C
	let mut candidates: Vec<(Id, Party)> = Vec::new();
	for (i, entry) in table.iter().enumerate() {
		if entry_has_secondary_key(entry, key_pool, key) {
			let dense_id = Id::from_u32(super::u32_index(i, registry_label));
			candidates.push((dense_id, parties[i]));
		}
	}

	if candidates.is_empty() {
		by_key.remove(&key);
		return;
	}

	// Find winner using total order
	let (mut winner_id, mut winner_party) = candidates[0];
	for (challenger_id, challenger_party) in candidates.into_iter().skip(1) {
		if table[challenger_id.as_u32() as usize]
			.total_order_cmp(table[winner_id.as_u32() as usize].as_ref())
			== Ordering::Greater
		{
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::SecondaryKey,
					incoming_kind: KeyKind::SecondaryKey,
					existing: winner_party,
					incoming: challenger_party,
					resolution: Resolution::ReplacedExisting,
				},
			});
			winner_id = challenger_id;
			winner_party = challenger_party;
		} else {
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::SecondaryKey,
					incoming_kind: KeyKind::SecondaryKey,
					existing: winner_party,
					incoming: challenger_party,
					resolution: Resolution::KeptExisting,
				},
			});
		}
	}

	by_key.insert(key, winner_id);
}
