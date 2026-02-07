use std::cmp::Ordering;
use std::sync::Arc;

use rustc_hash::FxHashMap as HashMap;

use super::types::RegistryIndex;
use crate::{
	Collision, CollisionKind, DenseId, DuplicatePolicy, FrozenInterner, InternerBuilder, KeyKind,
	Party, RegistryEntry, RegistrySource, Resolution, Symbol,
};

/// Borrowed metadata for building entries (supports both static and dynamic).
pub struct RegistryMetaRef<'a> {
	pub id: &'a str,
	pub name: &'a str,
	pub aliases: &'a [&'a str],
	pub description: &'a str,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'a [crate::Capability],
	pub flags: u32,
}

/// Trait for converting static or dynamic definitions into symbolized runtime entries.
pub trait BuildEntry<Out: RegistryEntry> {
	/// Returns the borrowed metadata.
	fn meta_ref(&self) -> RegistryMetaRef<'_>;
	/// Returns the short description string.
	fn short_desc_str(&self) -> &str;
	/// Collects all strings that need to be interned.
	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>);
	/// Converts to the symbolized runtime entry.
	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> Out;
}

/// Builder for constructing a [`RegistryIndex`].
pub struct RegistryBuilder<In, Out, Id>
where
	In: BuildEntry<Out> + Send + Sync + 'static,
	Out: RegistryEntry + Send + Sync + 'static,
	Id: DenseId,
{
	label: &'static str,
	defs: Vec<IngestEntry<In>>,
	policy: DuplicatePolicy,
	_marker: std::marker::PhantomData<(Out, Id)>,
}

struct IngestEntry<In> {
	ordinal: u32,
	inner: Arc<In>,
}

impl<In, Out, Id> RegistryBuilder<In, Out, Id>
where
	In: BuildEntry<Out> + Send + Sync + 'static,
	Out: RegistryEntry + Send + Sync + 'static,
	Id: DenseId,
{
	pub fn new(label: &'static str) -> Self {
		Self {
			label,
			defs: Vec::new(),
			policy: DuplicatePolicy::for_build(),
			_marker: std::marker::PhantomData,
		}
	}

	/// Creates a builder with an explicit duplicate policy (test-only).
	#[cfg(test)]
	pub fn with_policy(label: &'static str, policy: DuplicatePolicy) -> Self {
		Self {
			label,
			defs: Vec::new(),
			policy,
			_marker: std::marker::PhantomData,
		}
	}

	pub fn push(&mut self, def: Arc<In>) {
		let ordinal = self.defs.len() as u32;
		self.defs.push(IngestEntry {
			ordinal,
			inner: def,
		});
	}

	pub fn push_static(&mut self, def: &'static In)
	where
		In: Clone,
	{
		self.push(Arc::new(def.clone()));
	}

	/// Returns the number of definitions ingested so far.
	pub fn len(&self) -> usize {
		self.defs.len()
	}

	/// Returns true if no definitions have been ingested.
	pub fn is_empty(&self) -> bool {
		self.defs.is_empty()
	}

	pub fn extend<I: IntoIterator<Item = Arc<In>>>(&mut self, defs: I) {
		for def in defs {
			self.push(def);
		}
	}

	pub fn build(self) -> RegistryIndex<Out, Id> {
		// 1. Resolve ID duplicates (winners vs losers)
		let (winners, id_collisions) = resolve_id_duplicates(self.defs, self.policy);

		// 2. Build interner from WINNERS only
		let interner = build_interner(&winners);

		// 3. Build table and alias pool
		let (table, alias_pool, parties) = build_table(&winners, &interner);

		// 4. Build lookup map and resolve key conflicts
		let (by_key, key_collisions) =
			build_lookup(self.label, &table, &parties, &alias_pool, self.policy);

		// 5. Finalize collisions
		let mut all_collisions = Vec::with_capacity(id_collisions.len() + key_collisions.len());

		// Rehydrate ID collisions (now that we have an interner)
		for rec in id_collisions {
			let winner_party = parties
				.iter()
				.find(|p| p.ordinal == rec.winner_ordinal)
				.expect("winner party must exist");

			// The loser has the same ID string as the winner, so it must be interned.
			let loser_id_sym = interner
				.get(&rec.id_str)
				.expect("winner id string interned");

			let loser_party = Party {
				def_id: loser_id_sym,
				source: rec.loser_source,
				priority: rec.loser_priority,
				ordinal: rec.loser_ordinal,
			};

			all_collisions.push(Collision {
				registry: self.label,
				key: winner_party.def_id,
				kind: CollisionKind::DuplicateId {
					winner: *winner_party,
					loser: loser_party,
					policy: self.policy,
				},
			});
		}

		all_collisions.extend(key_collisions);

		RegistryIndex {
			table: Arc::from(table),
			by_key: Arc::new(by_key),
			interner,
			alias_pool: Arc::from(alias_pool),
			collisions: Arc::from(all_collisions),
		}
	}
}

pub(crate) fn resolve_id_duplicates<In, Out>(
	mut defs: Vec<IngestEntry<In>>,
	policy: DuplicatePolicy,
) -> (Vec<IngestEntry<In>>, Vec<IdCollisionRecord>)
where
	In: BuildEntry<Out>,
	Out: RegistryEntry,
{
	// Sort by canonical ID string for determinism, using ordinal as tie-break
	defs.sort_by(|a, b| {
		a.inner
			.meta_ref()
			.id
			.cmp(b.inner.meta_ref().id)
			.then_with(|| a.ordinal.cmp(&b.ordinal))
	});

	let mut winners = Vec::with_capacity(defs.len());
	let mut collisions = Vec::new();

	if !defs.is_empty() {
		let mut current_group = vec![&defs[0]];
		for def in &defs[1..] {
			if def.inner.meta_ref().id == current_group[0].inner.meta_ref().id {
				current_group.push(def);
			} else {
				let (winner, group_collisions) =
					resolve_winners_in_group::<In, Out>(&current_group, policy);
				winners.push(IngestEntry {
					ordinal: winner.ordinal,
					inner: winner.inner.clone(),
				});
				collisions.extend(group_collisions);
				current_group.clear();
				current_group.push(def);
			}
		}
		let (winner, group_collisions) =
			resolve_winners_in_group::<In, Out>(&current_group, policy);
		winners.push(IngestEntry {
			ordinal: winner.ordinal,
			inner: winner.inner.clone(),
		});
		collisions.extend(group_collisions);
	}

	(winners, collisions)
}

fn build_interner<In, Out>(winners: &[IngestEntry<In>]) -> FrozenInterner
where
	In: BuildEntry<Out>,
	Out: RegistryEntry,
{
	let mut interner_builder = InternerBuilder::new();
	let mut all_strings = Vec::new();

	for entry in winners {
		entry.inner.collect_strings(&mut all_strings);
	}
	all_strings.sort_unstable();
	all_strings.dedup();

	for s in all_strings {
		interner_builder.intern(s);
	}

	interner_builder.freeze()
}

fn build_table<In, Out>(
	winners: &[IngestEntry<In>],
	interner: &FrozenInterner,
) -> (Vec<std::sync::Arc<Out>>, Vec<Symbol>, Vec<Party>)
where
	In: BuildEntry<Out>,
	Out: RegistryEntry,
{
	let mut alias_pool = Vec::new();

	// Re-sort winners by ID string to ensure stable table indexing
	let mut sorted_winners: Vec<_> = winners.iter().collect();
	sorted_winners.sort_by(|a, b| a.inner.meta_ref().id.cmp(b.inner.meta_ref().id));

	let mut table = Vec::with_capacity(sorted_winners.len());
	let mut parties = Vec::with_capacity(sorted_winners.len());

	for entry in sorted_winners {
		let out = entry.inner.build(interner, &mut alias_pool);
		parties.push(Party {
			def_id: out.meta().id,
			source: out.meta().source,
			priority: out.meta().priority,
			ordinal: entry.ordinal,
		});
		table.push(std::sync::Arc::new(out));
	}

	(table, alias_pool, parties)
}

// Make this public so RuntimeRegistry can use it for extension
pub(crate) fn build_lookup<Out, Id>(
	registry_label: &'static str,
	table: &[std::sync::Arc<Out>],
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
		let current_party = parties[idx];

		if let Some(prev_id) = by_key.insert(sym, dense_id) {
			let prev_idx = prev_id.as_u32() as usize;
			collisions.push(Collision {
				registry: registry_label,
				key: sym,
				kind: CollisionKind::KeyConflict {
					existing_kind: KeyKind::Alias, // Must have been alias if we are in Stage A and prev existed? No, canonicals are unique.
					// Actually, if we had duplicate canonical IDs, resolve_id_duplicates would have caught them.
					// So this branch should theoretically be unreachable unless resolve_id_duplicates failed.
					// But let's keep the logic sound.
					incoming_kind: KeyKind::Canonical,
					existing: parties[prev_idx],
					incoming: current_party,
					resolution: Resolution::ReplacedExisting,
				},
			});
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
	a.priority()
		.cmp(&b.priority())
		.then_with(|| b.source().rank().cmp(&a.source().rank()))
}

struct IdCollisionRecord {
	id_str: String,
	winner_ordinal: u32,
	loser_ordinal: u32,
	loser_source: RegistrySource,
	loser_priority: i16,
}

fn resolve_winners_in_group<'a, In, Out>(
	group: &[&'a IngestEntry<In>],
	policy: DuplicatePolicy,
) -> (&'a IngestEntry<In>, Vec<IdCollisionRecord>)
where
	In: BuildEntry<Out>,
	Out: RegistryEntry,
{
	let mut winner = group[0];
	for &candidate in &group[1..] {
		let is_better = match policy {
			DuplicatePolicy::FirstWins => false,
			DuplicatePolicy::LastWins => true,
			DuplicatePolicy::ByPriority => {
				let a = winner.inner.meta_ref();
				let b = candidate.inner.meta_ref();
				b.priority
					.cmp(&a.priority)
					.then_with(|| a.source.rank().cmp(&b.source.rank()))
					.then_with(|| winner.ordinal.cmp(&candidate.ordinal))
					== Ordering::Greater
			}
			DuplicatePolicy::Panic => {
				panic!("Duplicate registry key: {}", winner.inner.meta_ref().id)
			}
		};
		if is_better {
			winner = candidate;
		}
	}

	let mut collision_recs = Vec::new();
	for &entry in group {
		if entry.ordinal != winner.ordinal {
			let meta = entry.inner.meta_ref();
			collision_recs.push(IdCollisionRecord {
				id_str: winner.inner.meta_ref().id.to_string(), // winner ID
				winner_ordinal: winner.ordinal,
				loser_ordinal: entry.ordinal,
				loser_source: meta.source,
				loser_priority: meta.priority,
			});
		}
	}

	(winner, collision_recs)
}
