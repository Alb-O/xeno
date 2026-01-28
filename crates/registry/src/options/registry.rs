use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap as HashMap;

use crate::RegistryEntry;
use crate::core::index::insert::insert_typed_key;
use crate::core::index::{
	ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore, RegistryIndex,
};
use crate::error::{InsertAction, RegistryError};
use crate::options::OptionDef;

#[derive(Clone)]
pub struct OptionsSnapshot {
	pub by_id: HashMap<&'static str, &'static OptionDef>,
	pub by_key: HashMap<&'static str, &'static OptionDef>,
	pub by_kdl: HashMap<&'static str, &'static OptionDef>,
	pub items_all: Vec<&'static OptionDef>,
	pub items_effective: Vec<&'static OptionDef>,
	pub collisions: Vec<Collision>,
}

impl OptionsSnapshot {
	fn from_builtins(b: &RegistryIndex<OptionDef>, policy: DuplicatePolicy) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_kdl: HashMap::default(),
			items_all: b.items_all.clone(),
			items_effective: b.items_effective.clone(),
			collisions: b.collisions.clone(),
		};

		for &def in b.items() {
			insert_kdl(&mut snap.by_kdl, def, policy);
		}
		snap
	}

	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static OptionDef> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	#[inline]
	pub fn by_kdl(&self, kdl_key: &str) -> Option<&'static OptionDef> {
		self.by_kdl.get(kdl_key).copied()
	}
}

fn insert_kdl(
	map: &mut HashMap<&'static str, &'static OptionDef>,
	def: &'static OptionDef,
	policy: DuplicatePolicy,
) {
	let key = def.kdl_key;
	match map.get(key).copied() {
		None => {
			map.insert(key, def);
		}
		Some(existing) => {
			if std::ptr::eq(existing, def) {
				return;
			}
			let new_wins = match policy {
				DuplicatePolicy::FirstWins => false,
				DuplicatePolicy::LastWins => true,
				DuplicatePolicy::ByPriority => def.total_order_cmp(existing) == Ordering::Greater,
				DuplicatePolicy::Panic => {
					panic!("duplicate option kdl_key {:?} for {}", key, def.id())
				}
			};
			if new_wins {
				map.insert(key, def);
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
	pub fn get(&self, key: &str) -> Option<&'static OptionDef> {
		self.snap.load().get(key)
	}

	#[inline]
	pub fn by_kdl_key(&self, kdl_key: &str) -> Option<&'static OptionDef> {
		self.snap.load().by_kdl(kdl_key)
	}

	pub fn try_register_many<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static OptionDef>,
	{
		self.try_register_many_internal(defs, false)
	}

	pub fn try_register_many_override<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static OptionDef>,
	{
		self.try_register_many_internal(defs, true)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static OptionDef>,
	{
		let input_defs: Vec<&'static OptionDef> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			let mut existing_ptrs: rustc_hash::FxHashSet<*const OptionDef> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &item in &next.items_all {
				existing_ptrs.insert(item as *const OptionDef);
			}

			let mut actions = Vec::with_capacity(input_defs.len());
			let choose_winner = self.make_choose_winner();

			{
				let mut store = SnapshotStore { snap: &mut next };

				for &def in &input_defs {
					if existing_ptrs.contains(&(def as *const OptionDef)) {
						actions.push(InsertAction::KeptExisting);
						continue;
					}

					let meta = def.meta();

					let id_action = if allow_overrides {
						insert_id_key_runtime(&mut store, "options", choose_winner, meta.id, def)?
					} else {
						insert_typed_key(
							&mut store,
							"options",
							choose_winner,
							KeyKind::Id,
							meta.id,
							def,
						)?
					};

					if allow_overrides && id_action == InsertAction::KeptExisting {
						actions.push(InsertAction::KeptExisting);
						continue;
					}

					let action = insert_typed_key(
						&mut store,
						"options",
						choose_winner,
						KeyKind::Name,
						meta.name,
						def,
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							"options",
							choose_winner,
							KeyKind::Alias,
							alias,
							def,
						)?;
					}

					insert_kdl(&mut store.snap.by_kdl, def, self.policy);
					store.snap.items_all.push(def);
					actions.push(action);
				}
			}

			let mut effective_set: rustc_hash::FxHashSet<*const OptionDef> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &def in next.by_id.values() {
				effective_set.insert(def as *const OptionDef);
			}
			for &def in next.by_key.values() {
				effective_set.insert(def as *const OptionDef);
			}
			next.items_effective = next
				.items_all
				.iter()
				.copied()
				.filter(|&d| effective_set.contains(&(d as *const OptionDef)))
				.collect();

			let next_arc = Arc::new(next);
			let prev = self.snap.compare_and_swap(&cur, next_arc);
			if Arc::ptr_eq(&prev, &cur) {
				return Ok(actions);
			}
		}
	}

	pub fn register(&self, def: &'static OptionDef) -> bool {
		self.try_register_many(std::iter::once(def)).is_ok()
	}

	pub fn register_owned(&self, def: OptionDef) -> bool {
		self.register(Box::leak(Box::new(def)))
	}

	pub fn try_register(&self, def: &'static OptionDef) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many(std::iter::once(def))?[0])
	}

	pub fn try_register_override(
		&self,
		def: &'static OptionDef,
	) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many_override(std::iter::once(def))?[0])
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

	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<&'static OptionDef> {
		self.with_snapshot(|snap| snap.by_id.get(id).copied())
	}

	#[inline]
	pub fn items(&self) -> Vec<&'static OptionDef> {
		self.with_snapshot(|snap| snap.items_effective.clone())
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.with_snapshot(|snap| snap.items_effective.len())
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	#[inline]
	pub fn collisions(&self) -> Vec<Collision> {
		self.with_snapshot(|snap| snap.collisions.clone())
	}

	pub fn iter(&self) -> Vec<&'static OptionDef> {
		self.items()
	}

	pub fn with_snapshot<R>(&self, f: impl FnOnce(&OptionsSnapshot) -> R) -> R {
		let snap = self.snap.load();
		f(&snap)
	}
}

fn insert_id_key_runtime(
	store: &mut SnapshotStore<'_>,
	registry_label: &'static str,
	choose_winner: ChooseWinner<OptionDef>,
	id: &'static str,
	def: &'static OptionDef,
) -> Result<InsertAction, RegistryError> {
	let existing = store.snap.by_id.get(id).copied();

	let Some(existing) = existing else {
		store.snap.by_id.insert(id, def);
		return Ok(InsertAction::InsertedNew);
	};

	if std::ptr::eq(existing, def) {
		return Ok(InsertAction::KeptExisting);
	}

	let new_wins = choose_winner(KeyKind::Id, id, existing, def);
	let (action, winner_id) = if new_wins {
		store.snap.by_id.insert(id, def);
		(InsertAction::ReplacedExisting, def.id())
	} else {
		(InsertAction::KeptExisting, existing.id())
	};

	store.snap.collisions.push(Collision {
		kind: KeyKind::Id,
		key: id,
		existing_id: existing.id(),
		new_id: def.id(),
		winner_id,
		action,
		registry: registry_label,
	});

	Ok(action)
}

struct SnapshotStore<'a> {
	snap: &'a mut OptionsSnapshot,
}

impl KeyStore<OptionDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<&'static OptionDef> {
		self.snap.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static OptionDef> {
		self.snap.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static OptionDef) {
		self.snap.by_key.insert(key, def);
	}

	fn insert_id(
		&mut self,
		id: &'static str,
		def: &'static OptionDef,
	) -> Option<&'static OptionDef> {
		match self.snap.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
