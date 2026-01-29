use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap as HashMap;

use crate::core::index::insert::{insert_id_key_runtime, insert_typed_key};
use crate::core::index::{
	ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore, RegistryIndex,
};
use crate::error::{InsertAction, RegistryError};
use crate::hooks::HookDef;
use crate::{HookEvent, RegistryEntry};

#[derive(Clone)]
pub struct HooksSnapshot {
	pub by_id: HashMap<&'static str, &'static HookDef>,
	pub by_key: HashMap<&'static str, &'static HookDef>,
	pub by_event: HashMap<HookEvent, Vec<&'static HookDef>>,
	pub items_all: Vec<&'static HookDef>,
	pub items_effective: Vec<&'static HookDef>,
	pub collisions: Vec<Collision>,
}

impl HooksSnapshot {
	fn from_builtins(b: &RegistryIndex<HookDef>) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_event: HashMap::default(),
			items_all: b.items_all.clone(),
			items_effective: b.items_effective.clone(),
			collisions: b.collisions.clone(),
		};

		for &def in b.items() {
			insert_hook_by_event(&mut snap.by_event, def);
		}
		snap
	}

	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static HookDef> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	#[inline]
	pub fn for_event(&self, event: HookEvent) -> &[&'static HookDef] {
		self.by_event.get(&event).map(Vec::as_slice).unwrap_or(&[])
	}
}

fn insert_hook_by_event(
	map: &mut HashMap<HookEvent, Vec<&'static HookDef>>,
	def: &'static HookDef,
) {
	let v = map.entry(def.event).or_default();
	let pos = v
		.binary_search_by(|h| h.total_order_cmp(def))
		.unwrap_or_else(|p| p);
	v.insert(pos, def);
}

pub struct HooksRegistry {
	#[allow(dead_code)]
	pub(super) builtins: RegistryIndex<HookDef>,
	pub(super) snap: ArcSwap<HooksSnapshot>,
	pub(super) policy: DuplicatePolicy,
}

impl HooksRegistry {
	pub fn new(builtins: RegistryIndex<HookDef>) -> Self {
		let snap = HooksSnapshot::from_builtins(&builtins);
		Self {
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy: DuplicatePolicy::for_build(),
		}
	}

	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static HookDef> {
		self.snap.load().get(key)
	}

	#[inline]
	pub fn for_event(&self, event: HookEvent) -> Vec<&'static HookDef> {
		self.snap.load().for_event(event).to_vec()
	}

	pub fn with_event<R>(&self, event: HookEvent, f: impl FnOnce(&[&'static HookDef]) -> R) -> R {
		self.with_snapshot(|snap| f(snap.for_event(event)))
	}

	pub fn try_register_many<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static HookDef>,
	{
		self.try_register_many_internal(defs, false)
	}

	pub fn try_register_many_override<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static HookDef>,
	{
		self.try_register_many_internal(defs, true)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static HookDef>,
	{
		let input_defs: Vec<&'static HookDef> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			let mut existing_ptrs: rustc_hash::FxHashSet<*const HookDef> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &item in &next.items_all {
				existing_ptrs.insert(item as *const HookDef);
			}

			let mut actions = Vec::with_capacity(input_defs.len());
			let choose_winner = self.make_choose_winner();

			{
				let mut store = SnapshotStore { snap: &mut next };

				for &def in &input_defs {
					if existing_ptrs.contains(&(def as *const HookDef)) {
						actions.push(InsertAction::KeptExisting);
						continue;
					}

					let meta = def.meta();

					let id_action = if allow_overrides {
						insert_id_key_runtime(&mut store, "hooks", choose_winner, meta.id, def)?
					} else {
						insert_typed_key(
							&mut store,
							"hooks",
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
						"hooks",
						choose_winner,
						KeyKind::Name,
						meta.name,
						def,
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							"hooks",
							choose_winner,
							KeyKind::Alias,
							alias,
							def,
						)?;
					}

					insert_hook_by_event(&mut store.snap.by_event, def);
					store.snap.items_all.push(def);
					actions.push(action);
				}
			}

			let mut effective_set: rustc_hash::FxHashSet<*const HookDef> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &def in next.by_id.values() {
				effective_set.insert(def as *const HookDef);
			}
			for &def in next.by_key.values() {
				effective_set.insert(def as *const HookDef);
			}
			next.items_effective = next
				.items_all
				.iter()
				.copied()
				.filter(|&d| effective_set.contains(&(d as *const HookDef)))
				.collect();

			let next_arc = Arc::new(next);
			let prev = self.snap.compare_and_swap(&cur, next_arc);
			if Arc::ptr_eq(&prev, &cur) {
				return Ok(actions);
			}
		}
	}

	pub fn register(&self, def: &'static HookDef) -> bool {
		self.try_register_many(std::iter::once(def)).is_ok()
	}

	pub fn register_owned(&self, def: HookDef) -> bool {
		self.register(Box::leak(Box::new(def)))
	}

	pub fn try_register(&self, def: &'static HookDef) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many(std::iter::once(def))?[0])
	}

	pub fn try_register_override(
		&self,
		def: &'static HookDef,
	) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many_override(std::iter::once(def))?[0])
	}

	fn make_choose_winner(&self) -> ChooseWinner<HookDef> {
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
	pub fn get_by_id(&self, id: &str) -> Option<&'static HookDef> {
		self.with_snapshot(|snap| snap.by_id.get(id).copied())
	}

	#[inline]
	pub fn all(&self) -> Vec<&'static HookDef> {
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

	pub fn iter(&self) -> Vec<&'static HookDef> {
		self.all()
	}

	pub fn items(&self) -> Vec<&'static HookDef> {
		self.all()
	}

	pub fn with_snapshot<R>(&self, f: impl FnOnce(&HooksSnapshot) -> R) -> R {
		let snap = self.snap.load();
		f(&snap)
	}
}

struct SnapshotStore<'a> {
	snap: &'a mut HooksSnapshot,
}

impl KeyStore<HookDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<&'static HookDef> {
		self.snap.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static HookDef> {
		self.snap.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static HookDef) {
		self.snap.by_key.insert(key, def);
	}

	fn insert_id(&mut self, id: &'static str, def: &'static HookDef) -> Option<&'static HookDef> {
		match self.snap.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn set_id_owner(&mut self, id: &'static str, def: &'static HookDef) {
		self.snap.by_id.insert(id, def);
	}

	fn evict_def(&mut self, def: &'static HookDef) {
		self.snap.by_key.retain(|_, &mut v| !std::ptr::eq(v, def));
		for hooks in self.snap.by_event.values_mut() {
			hooks.retain(|&v| !std::ptr::eq(v, def));
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
