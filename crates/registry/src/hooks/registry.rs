use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap as HashMap;

use crate::core::index::insert::{insert_id_key_runtime, insert_typed_key};
use crate::core::index::{
	ChooseWinner, Collision, DefPtr, DuplicatePolicy, KeyKind, KeyStore, RegistryIndex,
};
use crate::error::{InsertAction, RegistryError};
use crate::hooks::HookDef;
use crate::{HookEvent, RegistryEntry};

/// Guard object that keeps a hooks snapshot alive while providing access to a definition.
pub struct HooksRef {
	snap: Arc<HooksSnapshot>,
	ptr: DefPtr<HookDef>,
}

impl Clone for HooksRef {
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			ptr: self.ptr,
		}
	}
}

impl std::ops::Deref for HooksRef {
	type Target = HookDef;

	fn deref(&self) -> &HookDef {
		// Safety: The definition is kept alive by the snapshot Arc held in the guard.
		unsafe { self.ptr.as_ref() }
	}
}

#[derive(Clone)]
pub struct HooksSnapshot {
	pub by_id: HashMap<Box<str>, DefPtr<HookDef>>,
	pub by_key: HashMap<Box<str>, DefPtr<HookDef>>,
	pub by_event: HashMap<HookEvent, Vec<DefPtr<HookDef>>>,
	pub items_all: Vec<DefPtr<HookDef>>,
	pub items_effective: Vec<DefPtr<HookDef>>,
	/// Owns runtime-registered definitions so their pointers stay valid.
	pub owned: Vec<Arc<HookDef>>,
	pub collisions: Vec<Collision>,
}

impl HooksSnapshot {
	fn from_builtins(b: &RegistryIndex<HookDef>) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_event: HashMap::default(),
			items_all: b.items_all.to_vec(),
			items_effective: b.items_effective.to_vec(),
			owned: Vec::new(),
			collisions: b.collisions.to_vec(),
		};

		for &ptr in b.items() {
			insert_hook_by_event(&mut snap.by_event, ptr);
		}
		snap
	}

	#[inline]
	pub fn get_ptr(&self, key: &str) -> Option<DefPtr<HookDef>> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	#[inline]
	pub fn for_event(&self, event: HookEvent) -> &[DefPtr<HookDef>] {
		self.by_event.get(&event).map(Vec::as_slice).unwrap_or(&[])
	}
}

fn insert_hook_by_event(map: &mut HashMap<HookEvent, Vec<DefPtr<HookDef>>>, def: DefPtr<HookDef>) {
	let def_ref = unsafe { def.as_ref() };
	let v = map.entry(def_ref.event).or_default();
	let pos = v
		.binary_search_by(|h| {
			let h_ref = unsafe { h.as_ref() };
			h_ref.total_order_cmp(def_ref)
		})
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
		let ptr = snap.get_ptr(key)?;
		Some(HooksRef { snap, ptr })
	}

	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<HooksRef> {
		let snap = self.snap.load_full();
		let ptr = snap.by_id.get(id).copied()?;
		Some(HooksRef { snap, ptr })
	}

	pub fn all(&self) -> Vec<HooksRef> {
		let snap = self.snap.load_full();
		snap.items_effective
			.iter()
			.map(|&ptr| HooksRef {
				snap: snap.clone(),
				ptr,
			})
			.collect()
	}

	pub fn for_event(&self, event: HookEvent) -> Vec<HooksRef> {
		let snap = self.snap.load_full();
		snap.for_event(event)
			.iter()
			.map(|&ptr| HooksRef {
				snap: snap.clone(),
				ptr,
			})
			.collect()
	}

	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = HookDef>,
	{
		let owned: Vec<Arc<HookDef>> = defs.into_iter().map(Arc::new).collect();
		let ptrs: Vec<DefPtr<HookDef>> = owned.iter().map(|a| DefPtr::from_ref(&**a)).collect();
		self.try_register_many_internal(ptrs, owned, false)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		new_owned: Vec<Arc<HookDef>>,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefPtr<HookDef>>,
	{
		let input_defs: Vec<DefPtr<HookDef>> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			let mut existing_ptrs: rustc_hash::FxHashSet<DefPtr<HookDef>> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &item in &next.items_all {
				existing_ptrs.insert(item);
			}

			let mut actions = Vec::with_capacity(input_defs.len());
			let choose_winner = self.make_choose_winner();

			{
				let mut store = SnapshotStore { snap: &mut next };

				for &def in &input_defs {
					if existing_ptrs.contains(&def) {
						actions.push(InsertAction::KeptExisting);
						continue;
					}

					let meta = unsafe { def.as_ref() }.meta();

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

			next.owned.extend(new_owned.clone());

			// Prune
			{
				let mut referenced = rustc_hash::FxHashSet::default();
				for &ptr in next.by_id.values() {
					referenced.insert(ptr);
				}
				for &ptr in next.by_key.values() {
					referenced.insert(ptr);
				}
				next.owned
					.retain(|arc| referenced.contains(&DefPtr::from_ref(&**arc)));
			}

			let mut effective_set: rustc_hash::FxHashSet<DefPtr<HookDef>> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &def in next.by_id.values() {
				effective_set.insert(def);
			}
			for &def in next.by_key.values() {
				effective_set.insert(def);
			}
			next.items_effective = next
				.items_all
				.iter()
				.copied()
				.filter(|d| effective_set.contains(d))
				.collect();

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
		self.try_register_many_internal(std::iter::once(DefPtr::from_ref(def)), Vec::new(), false)
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
		self.snap.load().items_effective.len()
	}

	pub fn is_empty(&self) -> bool {
		self.snap.load().items_effective.is_empty()
	}
}

struct SnapshotStore<'a> {
	snap: &'a mut HooksSnapshot,
}

impl KeyStore<HookDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<DefPtr<HookDef>> {
		self.snap.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefPtr<HookDef>> {
		self.snap.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &str, def: DefPtr<HookDef>) {
		self.snap.by_key.insert(Box::from(key), def);
	}

	fn insert_id(&mut self, id: &str, def: DefPtr<HookDef>) -> Option<DefPtr<HookDef>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefPtr<HookDef>) {
		self.snap.by_id.insert(Box::from(id), def);
	}

	fn evict_def(&mut self, def: DefPtr<HookDef>) {
		self.snap.by_key.retain(|_, &mut v| !v.ptr_eq(def));
		for hooks in self.snap.by_event.values_mut() {
			hooks.retain(|&v| !v.ptr_eq(def));
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
