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
use crate::options::OptionDef;

/// Guard object that keeps an options snapshot alive while providing access to a definition.
pub struct OptionsRef {
	snap: Arc<OptionsSnapshot>,
	def: DefRef<OptionDef>,
}

impl Clone for OptionsRef {
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			def: self.def.clone(),
		}
	}
}

impl std::ops::Deref for OptionsRef {
	type Target = OptionDef;

	fn deref(&self) -> &OptionDef {
		self.def.as_entry()
	}
}

#[derive(Clone)]
pub struct OptionsSnapshot {
	pub by_id: HashMap<Box<str>, DefRef<OptionDef>>,
	pub by_key: HashMap<Box<str>, DefRef<OptionDef>>,
	pub by_kdl: HashMap<Box<str>, DefRef<OptionDef>>,
	pub id_order: Vec<Box<str>>,
	pub collisions: Vec<Collision>,
}

impl OptionsSnapshot {
	fn from_builtins(b: &RegistryIndex<OptionDef>, policy: DuplicatePolicy) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_kdl: HashMap::default(),
			id_order: b.id_order.clone(),
			collisions: b.collisions.to_vec(),
		};

		for def in b.iter() {
			if let Some(def_ref) = b.by_id.get(def.id()) {
				insert_kdl(&mut snap.by_kdl, def_ref.clone(), policy);
			}
		}
		snap
	}

	#[inline]
	pub fn get_def(&self, key: &str) -> Option<DefRef<OptionDef>> {
		self.by_id
			.get(key)
			.cloned()
			.or_else(|| self.by_key.get(key).cloned())
	}

	#[inline]
	pub fn by_kdl_def(&self, kdl_key: &str) -> Option<DefRef<OptionDef>> {
		self.by_kdl.get(kdl_key).cloned()
	}
}

fn insert_kdl(
	map: &mut HashMap<Box<str>, DefRef<OptionDef>>,
	def: DefRef<OptionDef>,
	policy: DuplicatePolicy,
) {
	let def_ref = def.as_entry();
	let key = def_ref.kdl_key;
	match map.get(key) {
		None => {
			map.insert(Box::from(key), def);
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
					panic!("duplicate option kdl_key {:?} for {}", key, def_ref.id())
				}
			};
			if new_wins {
				map.insert(Box::from(key), def);
			}
		}
	}
}

pub struct OptionsRegistry {
	#[allow(dead_code)]
	pub(super) builtins: RegistryIndex<OptionDef>,
	pub(super) snap: ArcSwap<OptionsSnapshot>,
	pub(super) policy: DuplicatePolicy,
}

impl OptionsRegistry {
	pub fn new(builtins: RegistryIndex<OptionDef>) -> Self {
		let policy = DuplicatePolicy::for_build();
		let snap = OptionsSnapshot::from_builtins(&builtins, policy);
		Self {
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy,
		}
	}

	#[inline]
	pub fn get(&self, key: &str) -> Option<OptionsRef> {
		let snap = self.snap.load_full();
		let def = snap.get_def(key)?;
		Some(OptionsRef { snap, def })
	}

	#[inline]
	pub fn by_kdl_key(&self, kdl_key: &str) -> Option<OptionsRef> {
		let snap = self.snap.load_full();
		let def = snap.by_kdl_def(kdl_key)?;
		Some(OptionsRef { snap, def })
	}

	pub fn items(&self) -> Vec<OptionsRef> {
		let snap = self.snap.load_full();
		snap.id_order
			.iter()
			.filter_map(|id| {
				let def = snap.by_id.get(id)?.clone();
				Some(OptionsRef {
					snap: snap.clone(),
					def,
				})
			})
			.collect()
	}

	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = OptionDef>,
	{
		self.try_register_many_internal(defs.into_iter().map(|d| DefRef::Owned(Arc::new(d))), false)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefRef<OptionDef>>,
	{
		let input_defs: Vec<DefRef<OptionDef>> = defs.into_iter().collect();
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
							"options",
							choose_winner,
							meta.id,
							def.clone(),
						)?
					} else {
						insert_typed_key(
							&mut store,
							"options",
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
						"options",
						choose_winner,
						KeyKind::Name,
						meta.name,
						def.clone(),
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							"options",
							choose_winner,
							KeyKind::Alias,
							alias,
							def.clone(),
						)?;
					}

					insert_kdl(&mut store.snap.by_kdl, def.clone(), self.policy);
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

	pub fn register_owned(&self, def: OptionDef) -> bool {
		self.try_register_many_owned(std::iter::once(def)).is_ok()
	}

	pub fn register(&self, def: &'static OptionDef) -> bool {
		self.try_register_many_internal(std::iter::once(DefRef::Builtin(def)), false)
			.is_ok()
	}

	fn make_choose_winner(&self) -> ChooseWinner<OptionDef> {
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

	pub fn with_snapshot<R>(&self, f: impl FnOnce(&OptionsSnapshot) -> R) -> R {
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

	pub fn get_by_id(&self, id: &str) -> Option<OptionsRef> {
		let snap = self.snap.load_full();
		let def = snap.by_id.get(id)?.clone();
		Some(OptionsRef { snap, def })
	}
}

struct SnapshotStore<'a> {
	snap: &'a mut OptionsSnapshot,
}

impl KeyStore<OptionDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<DefRef<OptionDef>> {
		self.snap.by_id.get(id).cloned()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefRef<OptionDef>> {
		self.snap.by_key.get(key).cloned()
	}

	fn set_key_winner(&mut self, key: &str, def: DefRef<OptionDef>) {
		self.snap.by_key.insert(Box::from(key), def);
	}

	fn insert_id(&mut self, id: &str, def: DefRef<OptionDef>) -> Option<DefRef<OptionDef>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(o.get().clone()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefRef<OptionDef>) {
		self.snap.by_id.insert(Box::from(id), def);
	}

	fn evict_def(&mut self, def: DefRef<OptionDef>) {
		self.snap.by_key.retain(|_, v| !v.ptr_eq(&def));
		self.snap.by_kdl.retain(|_, v| !v.ptr_eq(&def));
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
