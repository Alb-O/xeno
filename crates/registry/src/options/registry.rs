use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap as HashMap;

use crate::RegistryEntry;
use crate::core::index::insert::{insert_id_key_runtime, insert_typed_key};
use crate::core::index::{
	ChooseWinner, Collision, DefPtr, DuplicatePolicy, KeyKind, KeyStore, RegistryIndex,
};
use crate::error::{InsertAction, RegistryError};
use crate::options::OptionDef;

/// Guard object that keeps an options snapshot alive while providing access to a definition.
pub struct OptionsRef {
	snap: Arc<OptionsSnapshot>,
	ptr: DefPtr<OptionDef>,
}

impl Clone for OptionsRef {
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			ptr: self.ptr,
		}
	}
}

impl std::ops::Deref for OptionsRef {
	type Target = OptionDef;

	fn deref(&self) -> &OptionDef {
		// Safety: The definition is kept alive by the snapshot Arc held in the guard.
		unsafe { self.ptr.as_ref() }
	}
}

#[derive(Clone)]
pub struct OptionsSnapshot {
	pub by_id: HashMap<Box<str>, DefPtr<OptionDef>>,
	pub by_key: HashMap<Box<str>, DefPtr<OptionDef>>,
	pub by_kdl: HashMap<Box<str>, DefPtr<OptionDef>>,
	pub items_all: Vec<DefPtr<OptionDef>>,
	pub items_effective: Vec<DefPtr<OptionDef>>,
	/// Owns runtime-registered definitions so their pointers stay valid.
	pub owned: Vec<Arc<OptionDef>>,
	pub collisions: Vec<Collision>,
}

impl OptionsSnapshot {
	fn from_builtins(b: &RegistryIndex<OptionDef>, policy: DuplicatePolicy) -> Self {
		let mut snap = Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			by_kdl: HashMap::default(),
			items_all: b.items_all.to_vec(),
			items_effective: b.items_effective.to_vec(),
			owned: Vec::new(),
			collisions: b.collisions.to_vec(),
		};

		for &ptr in b.items() {
			insert_kdl(&mut snap.by_kdl, ptr, policy);
		}
		snap
	}

	#[inline]
	pub fn get_ptr(&self, key: &str) -> Option<DefPtr<OptionDef>> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	#[inline]
	pub fn by_kdl_ptr(&self, kdl_key: &str) -> Option<DefPtr<OptionDef>> {
		self.by_kdl.get(kdl_key).copied()
	}
}

fn insert_kdl(
	map: &mut HashMap<Box<str>, DefPtr<OptionDef>>,
	def: DefPtr<OptionDef>,
	policy: DuplicatePolicy,
) {
	let def_ref = unsafe { def.as_ref() };
	let key = def_ref.kdl_key;
	match map.get(key).copied() {
		None => {
			map.insert(Box::from(key), def);
		}
		Some(existing) => {
			if existing.ptr_eq(def) {
				return;
			}
			let existing_ref = unsafe { existing.as_ref() };
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
		let ptr = snap.get_ptr(key)?;
		Some(OptionsRef { snap, ptr })
	}

	#[inline]
	pub fn by_kdl_key(&self, kdl_key: &str) -> Option<OptionsRef> {
		let snap = self.snap.load_full();
		let ptr = snap.by_kdl_ptr(kdl_key)?;
		Some(OptionsRef { snap, ptr })
	}

	pub fn items(&self) -> Vec<OptionsRef> {
		let snap = self.snap.load_full();
		snap.items_effective
			.iter()
			.map(|&ptr| OptionsRef {
				snap: snap.clone(),
				ptr,
			})
			.collect()
	}

	pub fn try_register_many_owned<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = OptionDef>,
	{
		let owned: Vec<Arc<OptionDef>> = defs.into_iter().map(Arc::new).collect();
		let ptrs: Vec<DefPtr<OptionDef>> = owned.iter().map(|a| DefPtr::from_ref(&**a)).collect();
		self.try_register_many_internal(ptrs, owned, false)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		new_owned: Vec<Arc<OptionDef>>,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = DefPtr<OptionDef>>,
	{
		let input_defs: Vec<DefPtr<OptionDef>> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			let mut existing_ptrs: rustc_hash::FxHashSet<DefPtr<OptionDef>> =
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
				for &ptr in next.by_kdl.values() {
					referenced.insert(ptr);
				}
				next.owned
					.retain(|arc| referenced.contains(&DefPtr::from_ref(&**arc)));
			}

			let mut effective_set: rustc_hash::FxHashSet<DefPtr<OptionDef>> =
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

	pub fn register_owned(&self, def: OptionDef) -> bool {
		self.try_register_many_owned(std::iter::once(def)).is_ok()
	}

	pub fn register(&self, def: &'static OptionDef) -> bool {
		self.try_register_many_internal(std::iter::once(DefPtr::from_ref(def)), Vec::new(), false)
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
		self.snap.load().items_effective.len()
	}

	pub fn collisions(&self) -> Vec<Collision> {
		self.snap.load().collisions.clone()
	}

	pub fn get_by_id(&self, id: &str) -> Option<OptionsRef> {
		let snap = self.snap.load_full();
		let ptr = snap.by_id.get(id).copied()?;
		Some(OptionsRef { snap, ptr })
	}
}

struct SnapshotStore<'a> {
	snap: &'a mut OptionsSnapshot,
}

impl KeyStore<OptionDef> for SnapshotStore<'_> {
	fn get_id_owner(&self, id: &str) -> Option<DefPtr<OptionDef>> {
		self.snap.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<DefPtr<OptionDef>> {
		self.snap.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &str, def: DefPtr<OptionDef>) {
		self.snap.by_key.insert(Box::from(key), def);
	}

	fn insert_id(&mut self, id: &str, def: DefPtr<OptionDef>) -> Option<DefPtr<OptionDef>> {
		match self.snap.by_id.entry(Box::from(id)) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn set_id_owner(&mut self, id: &str, def: DefPtr<OptionDef>) {
		self.snap.by_id.insert(Box::from(id), def);
	}

	fn evict_def(&mut self, def: DefPtr<OptionDef>) {
		self.snap.by_key.retain(|_, &mut v| !v.ptr_eq(def));
		self.snap.by_kdl.retain(|_, &mut v| !v.ptr_eq(def));
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
