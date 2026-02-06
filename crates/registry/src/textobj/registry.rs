use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap as HashMap;

use crate::RegistryEntry;
use crate::core::index::insert::{insert_id_key_runtime, insert_typed_key};
use crate::core::index::{
	ChooseWinner, Collision, DefRef, DuplicatePolicy, KeyKind, KeyStore, RegistryIndex,
};
use crate::error::{InsertAction, RegistryError};
use crate::textobj::TextObjectDef;

/// Guard object that keeps a text-object snapshot alive while providing access to a definition.
pub struct TextObjectRef {
	snap: Arc<TextObjectSnapshot>,
	def: DefRef<TextObjectDef>,
}

impl Clone for TextObjectRef {
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			def: self.def.clone(),
		}
	}
}

impl std::ops::Deref for TextObjectRef {
	type Target = TextObjectDef;

	fn deref(&self) -> &TextObjectDef {
		self.def.as_entry()
	}
}

#[derive(Clone)]
pub struct TextObjectSnapshot {
	pub by_id: HashMap<Box<str>, DefRef<TextObjectDef>>,
	pub by_key: HashMap<Box<str>, DefRef<TextObjectDef>>,
	pub by_trigger: HashMap<char, DefRef<TextObjectDef>>,
	pub id_order: Vec<Box<str>>,
	pub collisions: Vec<Collision>,
}

impl TextObjectSnapshot {
	fn from_builtins(b: &RegistryIndex<TextObjectDef>, policy: DuplicatePolicy) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_trigger: HashMap::default(),
			id_order: b.id_order.clone(),
			collisions: b.collisions.to_vec(),
		};

		for def in b.iter() {
			if let Some(def_ref) = b.by_id.get(def.id()) {
				insert_trigger(&mut snap.by_trigger, def_ref.clone(), policy);
			}
		}
		snap
	}

	#[inline]
	pub fn get_def(&self, key: &str) -> Option<DefRef<TextObjectDef>> {
		self.by_id
			.get(key)
			.cloned()
			.or_else(|| self.by_key.get(key).cloned())
	}

	#[inline]
	pub fn by_trigger_def(&self, ch: char) -> Option<DefRef<TextObjectDef>> {
		self.by_trigger.get(&ch).cloned()
	}
}

fn insert_trigger(
	map: &mut HashMap<char, DefRef<TextObjectDef>>,
	def: DefRef<TextObjectDef>,
	policy: DuplicatePolicy,
) {
	let def_ref = def.as_entry();
	let mut insert_one = |ch: char| match map.get(&ch) {
		None => {
			map.insert(ch, def.clone());
		}
		Some(existing) => {
			if existing.ptr_eq(&def) {
				return;
			}
			let existing_ref = existing.as_entry();
			let new_wins = match policy {
				DuplicatePolicy::FirstWins => false,
				DuplicatePolicy::LastWins => true,
				DuplicatePolicy::ByPriority => {
					def_ref.total_order_cmp(existing_ref) == Ordering::Greater
				}
				DuplicatePolicy::Panic => {
					panic!(
						"duplicate text-object trigger {:?} for {}",
						ch,
						def_ref.id()
					)
				}
			};
			if new_wins {
				map.insert(ch, def.clone());
			}
		}
	};

	insert_one(def_ref.trigger);
	for &alt in def_ref.alt_triggers {
		insert_one(alt);
	}
}

pub struct TextObjectRegistry {
	#[allow(dead_code)]
	pub(super) builtins: RegistryIndex<TextObjectDef>,
	pub(super) snap: ArcSwap<TextObjectSnapshot>,
	pub(super) policy: DuplicatePolicy,
}

impl TextObjectRegistry {
	pub fn new(builtins: RegistryIndex<TextObjectDef>) -> Self {
		let policy = DuplicatePolicy::for_build();
		let snap = TextObjectSnapshot::from_builtins(&builtins, policy);
		Self {
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy,
		}
	}

	#[inline]
	pub fn get(&self, key: &str) -> Option<TextObjectRef> {
		let snap = self.snap.load_full();
		let def = snap.get_def(key)?;
		Some(TextObjectRef { snap, def })
	}

	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<TextObjectRef> {
		let snap = self.snap.load_full();
		let def = snap.by_id.get(id)?.clone();
		Some(TextObjectRef { snap, def })
	}

	#[inline]
	pub fn by_trigger(&self, ch: char) -> Option<TextObjectRef> {
		let snap = self.snap.load_full();
		let def = snap.by_trigger_def(ch)?;
		Some(TextObjectRef { snap, def })
	}

	pub fn all(&self) -> Vec<TextObjectRef> {
		let snap = self.snap.load_full();
		snap.id_order
			.iter()
			.filter_map(|id| {
				let def = snap.by_id.get(id)?.clone();
				Some(TextObjectRef {
					snap: snap.clone(),
					def,
				})
			})
			.collect()
	}

	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = TextObjectDef>,
	{
		self.try_register_many_internal(defs.into_iter().map(|d| DefRef::Owned(Arc::new(d))), false)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefRef<TextObjectDef>>,
	{
		let input_defs: Vec<DefRef<TextObjectDef>> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			let mut existing_identities = rustc_hash::FxHashSet::default();
			for def in cur.by_id.values() {
				existing_identities.insert(def.identity());
			}

			let mut actions = Vec::with_capacity(input_defs.len());
			let choose_winner = self.make_choose_winner();

			{
				let mut store = SnapshotStore { snap: &mut next };

				for def in &input_defs {
					if existing_identities.contains(&def.identity()) {
						actions.push(InsertAction::KeptExisting);
						continue;
					}

					let meta = def.as_entry().meta();

					let id_action = if allow_overrides {
						insert_id_key_runtime(
							&mut store,
							"text_objects",
							choose_winner,
							meta.id,
							def.clone(),
						)?
					} else {
						insert_typed_key(
							&mut store,
							"text_objects",
							choose_winner,
							KeyKind::Id,
							meta.id,
							def.clone(),
						)?
					};

					if allow_overrides && id_action == InsertAction::KeptExisting {
						actions.push(InsertAction::KeptExisting);
						continue;
					}

					if id_action == InsertAction::InsertedNew {
						store.snap.id_order.push(Box::from(meta.id));
					}

					let action = insert_typed_key(
						&mut store,
						"text_objects",
						choose_winner,
						KeyKind::Name,
						meta.name,
						def.clone(),
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							"text_objects",
							choose_winner,
							KeyKind::Alias,
							alias,
							def.clone(),
						)?;
					}

					insert_trigger(&mut store.snap.by_trigger, def.clone(), self.policy);
					actions.push(action);
				}
			}

			let next_arc = Arc::new(next);
			let prev = self.snap.compare_and_swap(&cur, next_arc);

			if Arc::ptr_eq(&prev, &cur) {
				return Ok(actions);
			}
		}
	}

	pub fn register_owned(&self, def: TextObjectDef) -> bool {
		self.try_register_many_owned(std::iter::once(def)).is_ok()
	}

	pub fn register(&self, def: &'static TextObjectDef) -> bool {
		self.try_register_many_internal(std::iter::once(DefRef::Builtin(def)), false)
			.is_ok()
	}

	fn make_choose_winner(&self) -> ChooseWinner<TextObjectDef> {
		match self.policy {
			DuplicatePolicy::Panic => |kind, key, existing, new| {
				panic!(
					"runtime registry key conflict: kind={} key={:?} existing_id={} new_id={}",
					kind,
					key,
					existing.id(),
					new.id()
				);
			},
			DuplicatePolicy::FirstWins => |_, _, _, _| false,
			DuplicatePolicy::LastWins => |_, _, _, _| true,
			DuplicatePolicy::ByPriority => {
				|_, _, existing, new| new.total_order_cmp(existing) == Ordering::Greater
			}
		}
	}

	pub fn with_snapshot<R>(&self, f: impl FnOnce(&TextObjectSnapshot) -> R) -> R {
		let snap = self.snap.load();
		f(&snap)
	}

	pub fn len(&self) -> usize {
		self.snap.load().id_order.len()
	}

	pub fn is_empty(&self) -> bool {
		self.snap.load().id_order.is_empty()
	}

	pub fn collisions(&self) -> Vec<Collision> {
		self.snap.load().collisions.clone()
	}
}

struct SnapshotStore<'a> {
	snap: &'a mut TextObjectSnapshot,
}

impl KeyStore<TextObjectDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<DefRef<TextObjectDef>> {
		self.snap.by_id.get(id).cloned()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefRef<TextObjectDef>> {
		self.snap.by_key.get(key).cloned()
	}

	fn set_key_winner(&mut self, key: &str, def: DefRef<TextObjectDef>) {
		self.snap.by_key.insert(Box::from(key), def);
	}

	fn insert_id(&mut self, id: &str, def: DefRef<TextObjectDef>) -> Option<DefRef<TextObjectDef>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(o.get().clone()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefRef<TextObjectDef>) {
		self.snap.by_id.insert(Box::from(id), def);
	}

	fn evict_def(&mut self, def: DefRef<TextObjectDef>) {
		self.snap.by_key.retain(|_, v| !v.ptr_eq(&def));
		self.snap.by_trigger.retain(|_, v| !v.ptr_eq(&def));
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
