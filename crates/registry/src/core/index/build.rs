use std::cmp::Ordering;
use std::sync::Arc;

use super::collision::{Collision, CollisionKind, DuplicatePolicy, Party, cmp_party};
use super::types::RegistryIndex;
use crate::{DenseId, FrozenInterner, InternerBuilder, RegistryEntry, RegistrySource, Symbol};

/// Context for interning and looking up strings during entry building.
pub trait BuildCtx {
	/// Interns a string and returns its symbol.
	fn intern(&mut self, s: &str) -> Symbol;
	/// Looks up a string and returns its symbol if it exists.
	fn get(&self, s: &str) -> Option<Symbol>;
	/// Resolves a symbol to its string representation.
	fn resolve(&self, sym: Symbol) -> &str;
}

/// Extension methods for BuildCtx to reduce boilerplate in domain implementations.
pub trait BuildCtxExt: BuildCtx {
	/// Interns a required string.
	fn intern_req(&mut self, s: &str, _what: &'static str) -> Symbol {
		self.intern(s)
	}

	/// Interns an optional string.
	fn intern_opt(&mut self, s: Option<&str>) -> Option<Symbol> {
		s.map(|s| self.intern(s))
	}

	/// Interns a slice of strings.
	fn intern_slice(&mut self, ss: &[&str]) -> Arc<[Symbol]> {
		ss.iter().map(|&s| self.intern(s)).collect::<Vec<_>>().into()
	}

	/// Interns an iterator of strings.
	fn intern_iter<I>(&mut self, it: I) -> Arc<[Symbol]>
	where
		I: IntoIterator,
		I::Item: AsRef<str>,
	{
		it.into_iter().map(|s| self.intern(s.as_ref())).collect::<Vec<_>>().into()
	}
}

impl<T: BuildCtx + ?Sized> BuildCtxExt for T {}

pub(crate) struct ProdBuildCtx<'a> {
	pub(crate) interner: &'a FrozenInterner,
}

impl BuildCtx for ProdBuildCtx<'_> {
	fn intern(&mut self, s: &str) -> Symbol {
		self.interner.get(s).expect("string not pre-interned in ProdBuildCtx")
	}

	fn get(&self, s: &str) -> Option<Symbol> {
		self.interner.get(s)
	}

	fn resolve(&self, sym: Symbol) -> &str {
		self.interner.resolve(sym)
	}
}

/// Instrumented context that verifies all used strings were collected.
#[cfg(any(debug_assertions, feature = "registry-contracts"))]
pub(crate) struct DebugBuildCtx<'a> {
	pub(crate) inner: &'a mut dyn BuildCtx,
	pub(crate) collected: std::collections::HashSet<&'a str>,
	pub(crate) used: std::collections::HashSet<String>,
}

#[cfg(any(debug_assertions, feature = "registry-contracts"))]
impl BuildCtx for DebugBuildCtx<'_> {
	fn intern(&mut self, s: &str) -> Symbol {
		if !self.collected.contains(s) {
			panic!("BuildEntry::build() interned string not in collect_strings(): '{}'", s);
		}
		self.used.insert(s.to_string());
		self.inner.intern(s)
	}

	fn get(&self, s: &str) -> Option<Symbol> {
		if !self.collected.contains(s) {
			panic!("BuildEntry::build() looked up string not in collect_strings(): '{}'", s);
		}
		// used.insert would need &mut self
		self.inner.get(s)
	}

	fn resolve(&self, sym: Symbol) -> &str {
		self.inner.resolve(sym)
	}
}

/// Helper for collecting strings to be interned.
pub struct StringCollector<'a, 'b>(pub &'a mut Vec<&'b str>);

impl<'a, 'b> StringCollector<'a, 'b> {
	/// Collects a single string.
	pub fn push(&mut self, s: &'b str) {
		self.0.push(s);
	}

	/// Collects an optional string.
	pub fn opt(&mut self, s: Option<&'b str>) {
		if let Some(s) = s {
			self.push(s);
		}
	}

	/// Collects an iterator of strings.
	pub fn extend<I>(&mut self, it: I)
	where
		I: IntoIterator<Item = &'b str>,
	{
		self.0.extend(it);
	}
}

/// Borrowed key list that works with both static (`&[&str]`) and owned (`&[String]`) storage.
pub enum StrListRef<'a> {
	/// Static string slices (from `RegistryMetaStatic`).
	Static(&'a [&'a str]),
	/// Owned strings (from registry-linked definitions).
	Owned(&'a [String]),
}

impl<'a> StrListRef<'a> {
	/// Calls `f` for each key string.
	pub fn for_each(&self, mut f: impl FnMut(&'a str)) {
		match self {
			Self::Static(xs) => xs.iter().copied().for_each(&mut f),
			Self::Owned(xs) => xs.iter().for_each(|s| f(s.as_str())),
		}
	}

	/// Collects all key strings into a `Vec<&str>`.
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
	pub keys: StrListRef<'a>,
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
	/// Collects strings for the payload only. Meta strings are handled by the builder.
	fn collect_payload_strings<'b>(&'b self, collector: &mut StringCollector<'_, 'b>);
	/// Converts to the symbolized runtime entry.
	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> Out;

	/// Collects all strings that need to be interned (used internally by the builder).
	fn collect_strings_all<'b>(&'b self, sink: &mut Vec<&'b str>) {
		let meta = self.meta_ref();
		let mut collector = StringCollector(sink);

		collector.push(meta.id);
		collector.push(meta.name);
		collector.push(meta.description);
		meta.keys.for_each(|k| collector.push(k));

		collector.push(self.short_desc_str());

		self.collect_payload_strings(&mut collector);
	}
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
		let ordinal = super::u32_index(self.defs.len(), self.label);
		self.defs.push(IngestEntry { ordinal, inner: def });
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
		use super::lookup::build_stage_maps;

		// 1. Resolve ID duplicates (winners vs losers)
		let (winners, id_collisions) = resolve_id_duplicates(self.defs, self.policy);

		// 2. Build interner from WINNERS only
		let interner = build_interner(&winners);

		// 3. Build table and key pool
		let (table, key_pool, parties) = build_table(&winners, &interner);

		// 4. Build canonical ID lookup (Stage A)
		let mut by_id = rustc_hash::FxHashMap::default();
		for (i, entry) in table.iter().enumerate() {
			by_id.insert(entry.meta().id, Id::from_u32(super::u32_index(i, self.label)));
		}

		// 5. Build Stage B and C maps with collision tracking
		let (by_name, by_key, key_collisions) = build_stage_maps(self.label, &table, &parties, &key_pool, &by_id);

		// 6. Finalize collisions
		let mut all_collisions = Vec::with_capacity(id_collisions.len() + key_collisions.len());

		// Rehydrate ID collisions (now that we have an interner)
		for rec in id_collisions {
			let winner_party = parties.iter().find(|p| p.ordinal == rec.winner_ordinal).expect("winner party must exist");

			// The loser has the same ID string as the winner, so it must be interned.
			let loser_id_sym = interner.get(&rec.id_str).expect("winner id string interned");

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
			by_id: Arc::new(by_id),
			by_name: Arc::new(by_name),
			by_key: Arc::new(by_key),
			interner,
			key_pool: Arc::from(key_pool),
			collisions: Arc::from(all_collisions),
			parties: Arc::from(parties),
		}
	}
}

pub(crate) fn resolve_id_duplicates<In, Out>(mut defs: Vec<IngestEntry<In>>, policy: DuplicatePolicy) -> (Vec<IngestEntry<In>>, Vec<IdCollisionRecord>)
where
	In: BuildEntry<Out>,
	Out: RegistryEntry,
{
	// Sort by canonical ID string for determinism, using ordinal as tie-break
	defs.sort_by(|a, b| a.inner.meta_ref().id.cmp(b.inner.meta_ref().id).then_with(|| a.ordinal.cmp(&b.ordinal)));

	let mut winners = Vec::with_capacity(defs.len());
	let mut collisions = Vec::new();

	if !defs.is_empty() {
		let mut current_group = vec![&defs[0]];
		for def in &defs[1..] {
			if def.inner.meta_ref().id == current_group[0].inner.meta_ref().id {
				current_group.push(def);
			} else {
				let (winner, group_collisions) = resolve_winners_in_group::<In, Out>(&current_group, policy);
				winners.push(IngestEntry {
					ordinal: winner.ordinal,
					inner: winner.inner.clone(),
				});
				collisions.extend(group_collisions);
				current_group.clear();
				current_group.push(def);
			}
		}
		let (winner, group_collisions) = resolve_winners_in_group::<In, Out>(&current_group, policy);
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
		entry.inner.collect_strings_all(&mut all_strings);
	}
	all_strings.sort_unstable();
	all_strings.dedup();

	for s in all_strings {
		interner_builder.intern(s);
	}

	interner_builder.freeze()
}

fn build_table<In, Out>(winners: &[IngestEntry<In>], interner: &FrozenInterner) -> (Vec<Arc<Out>>, Vec<Symbol>, Vec<Party>)
where
	In: BuildEntry<Out>,
	Out: RegistryEntry,
{
	let mut key_pool = Vec::new();

	// Re-sort winners by ID string to ensure stable table indexing
	let mut sorted_winners: Vec<_> = winners.iter().collect();
	sorted_winners.sort_by(|a, b| a.inner.meta_ref().id.cmp(b.inner.meta_ref().id));

	let mut table = Vec::with_capacity(sorted_winners.len());
	let mut parties = Vec::with_capacity(sorted_winners.len());

	for entry in sorted_winners {
		let mut prod_ctx = ProdBuildCtx { interner };

		#[cfg(any(debug_assertions, feature = "registry-contracts"))]
		let out = {
			let mut sink = Vec::new();
			entry.inner.collect_strings_all(&mut sink);
			let collected = sink.into_iter().collect();
			let mut ctx = DebugBuildCtx {
				inner: &mut prod_ctx,
				collected,
				used: std::collections::HashSet::default(),
			};
			entry.inner.build(&mut ctx, &mut key_pool)
		};

		#[cfg(not(any(debug_assertions, feature = "registry-contracts")))]
		let out = entry.inner.build(&mut prod_ctx, &mut key_pool);

		parties.push(Party {
			def_id: out.meta().id,
			source: out.meta().source,
			priority: out.meta().priority,
			ordinal: entry.ordinal,
		});
		table.push(Arc::new(out));
	}

	(table, key_pool, parties)
}

pub(crate) struct IdCollisionRecord {
	pub(crate) id_str: String,
	pub(crate) winner_ordinal: u32,
	pub(crate) loser_ordinal: u32,
	pub(crate) loser_source: RegistrySource,
	pub(crate) loser_priority: i16,
}

fn resolve_winners_in_group<'a, In, Out>(group: &[&'a IngestEntry<In>], policy: DuplicatePolicy) -> (&'a IngestEntry<In>, Vec<IdCollisionRecord>)
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
