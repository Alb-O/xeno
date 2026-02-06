use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap as HashMap;

use crate::core::index::insert::{insert_id_key_runtime, insert_typed_key};
use crate::core::index::{
	ChooseWinner, Collision, DefRef, DuplicatePolicy, KeyKind, KeyStore, RegistryIndex,
};
use crate::error::{InsertAction, RegistryError};
use crate::hooks::HookDef;
use crate::{HookEvent, RegistryEntry};

/// Guard object that keeps a hooks snapshot alive while providing access to a definition.
pub struct HooksRef {
	snap: Arc<HooksSnapshot>,
	def: DefRef<HookDef>,
}

impl Clone for HooksRef {
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			def: self.def.clone(),
		}
	}
}

impl std::ops::Deref for HooksRef {
	type Target = HookDef;

	fn deref(&self) -> &HookDef {
		self.def.as_entry()
	}
}

#[derive(Clone)]
pub struct HooksSnapshot {
	pub by_id: HashMap<Box<str>, DefRef<HookDef>>,
	pub by_key: HashMap<Box<str>, DefRef<HookDef>>,
	pub by_event: HashMap<HookEvent, Vec<DefRef<HookDef>>>,
	pub id_order: Vec<Box<str>>,
	pub collisions: Vec<Collision>,
}

impl HooksSnapshot {
	fn from_builtins(b: &RegistryIndex<HookDef>) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_event: HashMap::default(),
			id_order: b.id_order.clone(),
			collisions: b.collisions.to_vec(),
		};

		for def in b.iter() {
			// Need to find the DefRef for this def to insert into by_event.
			// Since RegistryIndex has by_id, we can find it there.
			if let Some(def_ref) = b.by_id.get(def.id()) {
				insert_hook_by_event(&mut snap.by_event, def_ref.clone());
			}
		}
		snap
	}

	#[inline]
	pub fn get_def(&self, key: &str) -> Option<DefRef<HookDef>> {
		self.by_id
			.get(key)
			.cloned()
			.or_else(|| self.by_key.get(key).cloned())
	}

	#[inline]
	pub fn for_event(&self, event: HookEvent) -> &[DefRef<HookDef>] {
		self.by_event.get(&event).map_or(&[], Vec::as_slice)
	}
}

fn insert_hook_by_event(map: &mut HashMap<HookEvent, Vec<DefRef<HookDef>>>, def: DefRef<HookDef>) {
	let def_ref = def.as_entry();
	let v = map.entry(def_ref.event).or_default();
	let pos = v
		.binary_search_by(|h| h.as_entry().total_order_cmp(def_ref))
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
	pub fn get(&self, key: &str) -> Option<HooksRef> {
		let snap = self.snap.load_full();
		let def = snap.get_def(key)?;
		Some(HooksRef { snap, def })
	}

	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<HooksRef> {
		let snap = self.snap.load_full();
		let def = snap.by_id.get(id)?.clone();
		Some(HooksRef { snap, def })
	}

	pub fn all(&self) -> Vec<HooksRef> {
		let snap = self.snap.load_full();
		snap.id_order
			.iter()
			.filter_map(|id| {
				let def = snap.by_id.get(id)?.clone();
				Some(HooksRef {
					snap: snap.clone(),
					def,
				})
			})
			.collect()
	}

	pub fn for_event(&self, event: HookEvent) -> Vec<HooksRef> {
		let snap = self.snap.load_full();
		snap.for_event(event)
			.iter()
			.map(|def| HooksRef {
				snap: snap.clone(),
				def: def.clone(),
			})
			.collect()
	}

	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = HookDef>,
	{
		self.try_register_many_internal(defs.into_iter().map(|d| DefRef::Owned(Arc::new(d))), false)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefRef<HookDef>>,
	{
		let input_defs: Vec<DefRef<HookDef>> = defs.into_iter().collect();
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
							"hooks",
							choose_winner,
							meta.id,
							def.clone(),
						)?
					} else {
						insert_typed_key(
							&mut store,
							"hooks",
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
						"hooks",
						choose_winner,
						KeyKind::Name,
						meta.name,
						def.clone(),
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							"hooks",
							choose_winner,
							KeyKind::Alias,
							alias,
							def.clone(),
						)?;
					}

					insert_hook_by_event(&mut store.snap.by_event, def.clone());
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

	pub fn register_owned(&self, def: HookDef) -> bool {
		self.try_register_many_owned(std::iter::once(def)).is_ok()
	}

	pub fn register(&self, def: &'static HookDef) -> bool {
		self.try_register_many_internal(std::iter::once(DefRef::Builtin(def)), false)
			.is_ok()
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

	pub fn with_snapshot<R>(&self, f: impl FnOnce(&HooksSnapshot) -> R) -> R {
		let snap = self.snap.load();
		f(&snap)
	}

	pub fn len(&self) -> usize {
		self.snap.load().id_order.len()
	}

	pub fn is_empty(&self) -> bool {
		self.snap.load().id_order.is_empty()
	}
}

struct SnapshotStore<'a> {
	snap: &'a mut HooksSnapshot,
}

impl KeyStore<HookDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<DefRef<HookDef>> {
		self.snap.by_id.get(id).cloned()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefRef<HookDef>> {
		self.snap.by_key.get(key).cloned()
	}

	fn set_key_winner(&mut self, key: &str, def: DefRef<HookDef>) {
		self.snap.by_key.insert(Box::from(key), def);
	}

	fn insert_id(&mut self, id: &str, def: DefRef<HookDef>) -> Option<DefRef<HookDef>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(o.get().clone()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefRef<HookDef>) {
		self.snap.by_id.insert(Box::from(id), def);
	}

	fn evict_def(&mut self, def: DefRef<HookDef>) {
		self.snap.by_key.retain(|_, v| !v.ptr_eq(&def));
		for hooks in self.snap.by_event.values_mut() {
			hooks.retain(|v| !v.ptr_eq(&def));
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
