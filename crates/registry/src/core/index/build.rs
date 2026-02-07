use std::cmp::Ordering;
use std::sync::Arc;

use super::collision::{Collision, CollisionKind, DuplicatePolicy, Party, cmp_party};
use super::types::RegistryIndex;
use crate::{DenseId, FrozenInterner, InternerBuilder, RegistryEntry, RegistrySource, Symbol};

/// Borrowed alias list that works with both static (`&[&str]`) and owned (`&[String]`) storage.
pub enum StrListRef<'a> {
	/// Static string slices (from `RegistryMetaStatic`).
	Static(&'a [&'a str]),
	/// Owned strings (from KDL-linked definitions).
	Owned(&'a [String]),
}

impl<'a> StrListRef<'a> {
	/// Calls `f` for each alias string.
	pub fn for_each(&self, mut f: impl FnMut(&'a str)) {
		match self {
			Self::Static(xs) => xs.iter().copied().for_each(&mut f),
			Self::Owned(xs) => xs.iter().for_each(|s| f(s.as_str())),
		}
	}

	/// Collects all alias strings into a `Vec<&str>`.
	pub fn to_vec(&self) -> Vec<&'a str> {
		let mut out = Vec::new();
		self.for_each(|s| out.push(s));
		out
	}
}

/// Borrowed metadata for building entries (supports both static and dynamic).
pub struct RegistryMetaRef<'a> {
	pub id: &'a str,
	pub name: &'a str,
	pub aliases: StrListRef<'a>,
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

pub(crate) struct IngestEntry<In> {
	pub(crate) ordinal: u32,
	pub(crate) inner: Arc<In>,
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
		use super::lookup::build_lookup;

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
) -> (Vec<Arc<Out>>, Vec<Symbol>, Vec<Party>)
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
		table.push(Arc::new(out));
	}

	(table, alias_pool, parties)
}

pub(crate) struct IdCollisionRecord {
	pub(crate) id_str: String,
	pub(crate) winner_ordinal: u32,
	pub(crate) loser_ordinal: u32,
	pub(crate) loser_source: RegistrySource,
	pub(crate) loser_priority: i16,
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
				let a_meta = winner.inner.meta_ref();
				let b_meta = candidate.inner.meta_ref();

				let a_party = Party {
					def_id: Symbol::INVALID,
					source: a_meta.source,
					priority: a_meta.priority,
					ordinal: winner.ordinal,
				};
				let b_party = Party {
					def_id: Symbol::INVALID,
					source: b_meta.source,
					priority: b_meta.priority,
					ordinal: candidate.ordinal,
				};

				cmp_party(&b_party, &a_party) == Ordering::Greater
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
