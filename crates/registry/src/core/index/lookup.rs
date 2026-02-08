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

use rustc_hash::FxHashMap;

use crate::core::{
	Collision, CollisionKind, DenseId, DuplicatePolicy, KeyKind, Party, RegistryEntry, Resolution,
	Snapshot, Symbol,
};

pub(crate) type Map<K, V> = FxHashMap<K, V>;

pub(crate) fn collect_entry_keys<T: RegistryEntry>(
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

pub(crate) fn get_key_kind<T, Id>(
	key: Symbol,
	dense_id: Id,
	table: &[Arc<T>],
	by_id: &FxHashMap<Symbol, Id>,
) -> KeyKind
where
	T: RegistryEntry,
	Id: DenseId,
{
	if let Some(&id) = by_id.get(&key)
		&& id == dense_id
	{
		KeyKind::Canonical
	} else if table[dense_id.as_u32() as usize].name() == key {
		KeyKind::PrimaryName
	} else {
		KeyKind::SecondaryKey
	}
}

pub(crate) fn update_lookup_append<T, Id>(
	registry_label: &'static str,
	table: &[Arc<T>],
	parties: &[Party],
	key_pool: &[Symbol],
	_policy: DuplicatePolicy,
	new_idx: usize,
	old_snap: &Snapshot<T, Id>,
	new_by_id: &FxHashMap<Symbol, Id>,
) -> (FxHashMap<Symbol, Id>, Vec<Collision>)
where
	T: super::RuntimeEntry,
	Id: DenseId,
{
	let mut by_key = (*old_snap.by_key).clone();
	let mut collisions = old_snap.collisions.to_vec();

	let entry = &table[new_idx];
	let dense_id = Id::from_u32(super::u32_index(new_idx, registry_label));
	let current_party = parties[new_idx];

	// Stage A: Canonical ID
	let id_sym = entry.id();
	by_key.insert(id_sym, dense_id);

	// Stage B: Primary Name
	let name_sym = entry.name();
	resolve_incremental(
		registry_label,
		name_sym,
		dense_id,
		current_party,
		KeyKind::PrimaryName,
		&mut by_key,
		&mut collisions,
		table,
		parties,
		new_by_id,
	);

	// Stage C: Secondary Keys
	let meta = entry.meta();
	let start = meta.keys.start as usize;
	let len = meta.keys.len as usize;
	let secondary_keys = &key_pool[start..start + len];

	for &key in secondary_keys {
		resolve_incremental(
			registry_label,
			key,
			dense_id,
			current_party,
			KeyKind::SecondaryKey,
			&mut by_key,
			&mut collisions,
			table,
			parties,
			new_by_id,
		);
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_key, collisions)
}

pub(crate) fn update_lookup_replace<T, Id>(
	registry_label: &'static str,
	table: &[Arc<T>],
	parties: &[Party],
	key_pool: &[Symbol],
	_policy: DuplicatePolicy,
	replaced_idx: usize,
	old_snap: &Snapshot<T, Id>,
	new_by_id: &FxHashMap<Symbol, Id>,
) -> (FxHashMap<Symbol, Id>, Vec<Collision>)
where
	T: super::RuntimeEntry,
	Id: DenseId,
{
	let mut by_key = (*old_snap.by_key).clone();

	let mut affected_keys = std::collections::HashSet::default();
	collect_entry_keys(
		old_snap.table[replaced_idx].as_ref(),
		old_snap.key_pool.as_ref(),
		&mut affected_keys,
	);
	collect_entry_keys(table[replaced_idx].as_ref(), key_pool, &mut affected_keys);

	// Filter out collisions involving the replaced entry
	let replaced_ordinal = old_snap.parties[replaced_idx].ordinal;
	let mut collisions: Vec<Collision> = old_snap
		.collisions
		.iter()
		.filter(|c| match &c.kind {
			CollisionKind::KeyConflict {
				existing, incoming, ..
			} => existing.ordinal != replaced_ordinal && incoming.ordinal != replaced_ordinal,
			CollisionKind::DuplicateId { winner, loser, .. } => {
				winner.ordinal != replaced_ordinal && loser.ordinal != replaced_ordinal
			}
		})
		.cloned()
		.collect();

	for key in affected_keys {
		recalculate_key_winner(
			registry_label,
			key,
			&mut by_key,
			&mut collisions,
			table,
			parties,
			key_pool,
			new_by_id,
		);
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_key, collisions)
}

fn recalculate_key_winner<T, Id>(
	registry_label: &'static str,
	key: Symbol,
	by_key: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
	table: &[Arc<T>],
	parties: &[Party],
	key_pool: &[Symbol],
	_by_id: &FxHashMap<Symbol, Id>,
) where
	T: RegistryEntry,
	Id: DenseId,
{
	let mut candidates = Vec::new();
	for (i, entry) in table.iter().enumerate() {
		let kind = if entry.id() == key {
			Some(KeyKind::Canonical)
		} else if entry.name() == key {
			Some(KeyKind::PrimaryName)
		} else {
			let meta = entry.meta();
			let start = meta.keys.start as usize;
			let len = meta.keys.len as usize;
			if key_pool[start..start + len].contains(&key) {
				Some(KeyKind::SecondaryKey)
			} else {
				None
			}
		};

		if let Some(k) = kind {
			let dense_id = Id::from_u32(super::u32_index(i, registry_label));
			candidates.push((dense_id, parties[i], k));
		}
	}

	if candidates.is_empty() {
		by_key.remove(&key);
		return;
	}

	// Candidates are already in table order. Simulate build_lookup loop.
	let (first_id, first_party, first_kind) = candidates[0];
	let mut winner_id = first_id;
	let mut winner_party = first_party;
	let mut winner_kind = first_kind;

	for i in 1..candidates.len() {
		let (challenger_id, challenger_party, challenger_kind) = candidates[i];

		let challenger_better = match (winner_kind, challenger_kind) {
			(KeyKind::Canonical, _) => false,
			(_, KeyKind::Canonical) => true,
			(KeyKind::PrimaryName, KeyKind::PrimaryName)
			| (KeyKind::SecondaryKey, KeyKind::SecondaryKey) => {
				table[challenger_id.as_u32() as usize]
					.total_order_cmp(table[winner_id.as_u32() as usize].as_ref())
					== Ordering::Greater
			}
			(KeyKind::PrimaryName, KeyKind::SecondaryKey) => false,
			(KeyKind::SecondaryKey, KeyKind::PrimaryName) => true,
		};

		if challenger_better {
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: winner_kind,
					incoming_kind: challenger_kind,
					existing: winner_party,
					incoming: challenger_party,
					resolution: Resolution::ReplacedExisting,
				},
			});
			winner_id = challenger_id;
			winner_party = challenger_party;
			winner_kind = challenger_kind;
		} else {
			collisions.push(Collision {
				registry: registry_label,
				key,
				kind: CollisionKind::KeyConflict {
					existing_kind: winner_kind,
					incoming_kind: challenger_kind,
					existing: winner_party,
					incoming: challenger_party,
					resolution: Resolution::KeptExisting,
				},
			});
		}
	}

	by_key.insert(key, winner_id);
}

fn resolve_incremental<T, Id>(
	registry_label: &'static str,
	key: Symbol,
	incoming_id: Id,
	incoming_party: Party,
	incoming_kind: KeyKind,
	by_key: &mut FxHashMap<Symbol, Id>,
	collisions: &mut Vec<Collision>,
	table: &[Arc<T>],
	parties: &[Party],
	by_id: &FxHashMap<Symbol, Id>,
) where
	T: super::RuntimeEntry,
	Id: DenseId,
{
	if let Some(&existing_id) = by_key.get(&key) {
		if existing_id == incoming_id {
			return;
		}

		let existing_idx = existing_id.as_u32() as usize;
		let existing_party = parties[existing_idx];
		let existing_kind = get_key_kind(key, existing_id, table, by_id);

		match (existing_kind, incoming_kind) {
			(KeyKind::Canonical, _) => {
				// Stage A wins
				collisions.push(Collision {
					registry: registry_label,
					key,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::Canonical,
						incoming_kind,
						existing: existing_party,
						incoming: incoming_party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
			(KeyKind::PrimaryName, KeyKind::PrimaryName)
			| (KeyKind::SecondaryKey, KeyKind::SecondaryKey) => {
				// Same stage: compare entries
				if compare_out(
					table[incoming_id.as_u32() as usize].as_ref(),
					table[existing_idx].as_ref(),
				) == Ordering::Greater
				{
					by_key.insert(key, incoming_id);
					collisions.push(Collision {
						registry: registry_label,
						key,
						kind: CollisionKind::KeyConflict {
							existing_kind,
							incoming_kind,
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
							existing_kind,
							incoming_kind,
							existing: existing_party,
							incoming: incoming_party,
							resolution: Resolution::KeptExisting,
						},
					});
				}
			}
			(KeyKind::PrimaryName, KeyKind::SecondaryKey) => {
				// Stage B wins over Stage C
				collisions.push(Collision {
					registry: registry_label,
					key,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::PrimaryName,
						incoming_kind: KeyKind::SecondaryKey,
						existing: existing_party,
						incoming: incoming_party,
						resolution: Resolution::KeptExisting,
					},
				});
			}
			(KeyKind::SecondaryKey, KeyKind::PrimaryName) => {
				// Stage B displaces Stage C
				by_key.insert(key, incoming_id);
				collisions.push(Collision {
					registry: registry_label,
					key,
					kind: CollisionKind::KeyConflict {
						existing_kind: KeyKind::SecondaryKey,
						incoming_kind: KeyKind::PrimaryName,
						existing: existing_party,
						incoming: incoming_party,
						resolution: Resolution::ReplacedExisting,
					},
				});
			}
			(_, KeyKind::Canonical) => {
				// Canonical Stage A displaces any prior mapping
				by_key.insert(key, incoming_id);
			}
		}
	} else {
		by_key.insert(key, incoming_id);
	}
}

pub(crate) fn compare_out<T: RegistryEntry>(a: &T, b: &T) -> Ordering {
	// Use the established total order from RegistryEntry.
	// This ensures consistency across all conflict resolution points.
	a.total_order_cmp(b)
}

// Make this public so RuntimeRegistry can use it for extension
pub(crate) fn build_lookup<Out, Id>(
	registry_label: &'static str,
	table: &[Arc<Out>],
	parties: &[Party],
	key_pool: &[Symbol],
	_policy: DuplicatePolicy,
) -> (FxHashMap<Symbol, Id>, Vec<Collision>)
where
	Out: super::RuntimeEntry,
	Id: DenseId,
{
	let mut by_key = FxHashMap::default();
	let mut collisions = Vec::new();

	// Stage A: Canonical IDs
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		by_key.insert(entry.id(), dense_id);
	}

	// Internal by_id map for kind detection during build
	let mut by_id = FxHashMap::default();
	for (idx, entry) in table.iter().enumerate() {
		by_id.insert(
			entry.id(),
			Id::from_u32(super::u32_index(idx, registry_label)),
		);
	}

	// Stage B: Primary Names
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		resolve_incremental(
			registry_label,
			entry.name(),
			dense_id,
			parties[idx],
			KeyKind::PrimaryName,
			&mut by_key,
			&mut collisions,
			table,
			parties,
			&by_id,
		);
	}

	// Stage C: Secondary Keys
	for (idx, entry) in table.iter().enumerate() {
		let dense_id = Id::from_u32(super::u32_index(idx, registry_label));
		let meta = entry.meta();
		let start = meta.keys.start as usize;
		let len = meta.keys.len as usize;
		for &key in &key_pool[start..start + len] {
			resolve_incremental(
				registry_label,
				key,
				dense_id,
				parties[idx],
				KeyKind::SecondaryKey,
				&mut by_key,
				&mut collisions,
				table,
				parties,
				&by_id,
			);
		}
	}

	collisions.sort_by(Collision::stable_cmp);
	(by_key, collisions)
}
