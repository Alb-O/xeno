use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;

use super::collision::{ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore};
use super::insert::{insert_id_key_runtime, insert_typed_key};
use super::types::{Map, RegistryIndex};
use crate::RegistryEntry;
use crate::error::{InsertAction, RegistryError};

/// Single source of truth for registry lookups.
///
/// Contains a merged view of builtins and runtime extensions.
pub struct Snapshot<T: RegistryEntry + 'static> {
	pub by_id: Map<&'static str, &'static T>,
	pub by_key: Map<&'static str, &'static T>,
	pub items_all: Vec<&'static T>,
	pub items_effective: Vec<&'static T>,
	pub collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> Clone for Snapshot<T> {
	fn clone(&self) -> Self {
		Self {
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			items_all: self.items_all.clone(),
			items_effective: self.items_effective.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T: RegistryEntry + 'static> Snapshot<T> {
	/// Creates a new snapshot from a builtin index.
	fn from_builtins(b: &RegistryIndex<T>) -> Self {
		Self {
			by_id: b.by_id.clone(),
			by_key: b.by_key.clone(),
			items_all: b.items_all.clone(),
			items_effective: b.items_effective.clone(),
			collisions: b.collisions.clone(),
		}
	}

	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static T> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<&'static T> {
		self.by_id.get(id).copied()
	}
}

/// Registry wrapper for runtime-extensible registries.
pub struct RuntimeRegistry<T: RegistryEntry + 'static> {
	pub(super) label: &'static str,
	pub(super) builtins: RegistryIndex<T>,
	pub(super) snap: ArcSwap<Snapshot<T>>,
	pub(super) policy: DuplicatePolicy,
}

impl<T: RegistryEntry + 'static> RuntimeRegistry<T> {
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T>) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self {
			label,
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy: DuplicatePolicy::for_build(),
		}
	}

	/// Creates a new runtime registry with a custom duplicate policy.
	pub fn with_policy(
		label: &'static str,
		builtins: RegistryIndex<T>,
		policy: DuplicatePolicy,
	) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self {
			label,
			builtins,
			snap: ArcSwap::from_pointee(snap),
			policy,
		}
	}

	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static T> {
		self.snap.load().get(key)
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<&'static T> {
		self.snap.load().get_by_id(id)
	}

	/// Registers a definition at runtime.
	pub fn register(&self, def: &'static T) -> bool {
		self.try_register(def).is_ok()
	}

	/// Registers an owned definition by leaking it.
	pub fn register_owned(&self, def: T) -> bool {
		self.register(Box::leak(Box::new(def)))
	}

	/// Registers many definitions at runtime in a single atomic operation.
	pub fn register_many<I>(&self, defs: I) -> Result<usize, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		Ok(self.try_register_many(defs)?.len())
	}

	/// Registers many owned definitions by leaking them.
	pub fn register_many_owned<I>(&self, defs: I) -> Result<usize, RegistryError>
	where
		I: IntoIterator<Item = T>,
	{
		let leaked: Vec<&'static T> = defs.into_iter().map(|d| &*Box::leak(Box::new(d))).collect();
		self.register_many(leaked)
	}

	/// Attempts to register many definitions at runtime in a single atomic operation.
	pub fn try_register_many<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		self.try_register_many_internal(defs, false)
	}

	/// Attempts to register many definitions with ID override support.
	pub fn try_register_many_override<I>(&self, defs: I) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		self.try_register_many_internal(defs, true)
	}

	fn try_register_many_internal<I>(
		&self,
		defs: I,
		allow_overrides: bool,
	) -> Result<Vec<InsertAction>, RegistryError>
	where
		I: IntoIterator<Item = &'static T>,
	{
		let input_defs: Vec<&'static T> = defs.into_iter().collect();
		if input_defs.is_empty() {
			return Ok(Vec::new());
		}

		loop {
			let cur = self.snap.load_full();
			let mut next = (*cur).clone();

			// Build pointer set of already registered items for efficient dedup
			let mut existing_ptrs: rustc_hash::FxHashSet<*const T> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &item in &next.items_all {
				existing_ptrs.insert(item as *const T);
			}

			let mut new_defs_indices = Vec::with_capacity(input_defs.len());
			for (idx, &def) in input_defs.iter().enumerate() {
				if !existing_ptrs.contains(&(def as *const T)) {
					new_defs_indices.push(idx);
				}
			}

			if new_defs_indices.is_empty() {
				return Ok(vec![InsertAction::KeptExisting; input_defs.len()]);
			}

			let mut actions = vec![InsertAction::KeptExisting; input_defs.len()];
			let choose_winner = self.make_choose_winner();

			{
				let mut store = SnapshotStore { snap: &mut next };

				for idx in new_defs_indices {
					let def = input_defs[idx];
					let meta = def.meta();

					let id_action = if allow_overrides {
						insert_id_key_runtime(&mut store, self.label, choose_winner, meta.id, def)?
					} else {
						insert_typed_key(
							&mut store,
							self.label,
							choose_winner,
							KeyKind::Id,
							meta.id,
							def,
						)?
					};

					// If we're overriding and we lost the ID contest, skip this item entirely
					if allow_overrides && id_action == InsertAction::KeptExisting {
						actions[idx] = InsertAction::KeptExisting;
						continue;
					}

					let action = insert_typed_key(
						&mut store,
						self.label,
						choose_winner,
						KeyKind::Name,
						meta.name,
						def,
					)?;

					for &alias in meta.aliases {
						insert_typed_key(
							&mut store,
							self.label,
							choose_winner,
							KeyKind::Alias,
							alias,
							def,
						)?;
					}

					store.snap.items_all.push(def);
					actions[idx] = action;
				}
			}

			// Update items_effective
			let mut effective_set: rustc_hash::FxHashSet<*const T> =
				rustc_hash::FxHashSet::with_capacity_and_hasher(
					next.items_all.len(),
					Default::default(),
				);
			for &def in next.by_id.values() {
				effective_set.insert(def as *const T);
			}
			for &def in next.by_key.values() {
				effective_set.insert(def as *const T);
			}
			next.items_effective = next
				.items_all
				.iter()
				.copied()
				.filter(|&d| effective_set.contains(&(d as *const T)))
				.collect();

			let next_arc = Arc::new(next);
			let prev = self.snap.compare_and_swap(&cur, next_arc);

			if Arc::ptr_eq(&prev, &cur) {
				return Ok(actions);
			}
			// CAS failed, retry
		}
	}

	/// Attempts to register a definition at runtime, returning detailed error info.
	pub fn try_register(&self, def: &'static T) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many(std::iter::once(def))?[0])
	}

	/// Attempts to register a definition with ID override support.
	pub fn try_register_override(&self, def: &'static T) -> Result<InsertAction, RegistryError> {
		Ok(self.try_register_many_override(std::iter::once(def))?[0])
	}

	fn make_choose_winner(&self) -> ChooseWinner<T> {
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

	/// Returns the number of unique definitions (builtins + extras).
	pub fn len(&self) -> usize {
		self.snap.load().items_effective.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.snap.load().items_effective.is_empty()
	}

	/// Returns all definitions (builtins followed by extras).
	pub fn all(&self) -> Vec<&'static T> {
		self.snap.load().items_effective.clone()
	}

	/// Returns definitions added at runtime.
	pub fn extras_items(&self) -> Vec<&'static T> {
		let builtins_set: rustc_hash::FxHashSet<*const T> = rustc_hash::FxHashSet::from_iter(
			self.builtins.items_all().iter().map(|&b| b as *const T),
		);
		self.snap
			.load()
			.items_all
			.iter()
			.copied()
			.filter(|&d| !builtins_set.contains(&(d as *const T)))
			.collect()
	}

	/// Returns the underlying builtins index.
	pub fn builtins(&self) -> &RegistryIndex<T> {
		&self.builtins
	}

	/// Returns an iterator over effective definitions.
	pub fn iter(&self) -> impl Iterator<Item = &'static T> + '_ {
		self.all().into_iter()
	}

	/// Returns the effective items slice.
	pub fn items(&self) -> Vec<&'static T> {
		self.all()
	}

	/// Returns all recorded collisions (builtins + runtime).
	pub fn collisions(&self) -> Vec<Collision> {
		self.snap.load().collisions.clone()
	}

	/// Returns the current snapshot guard so callers can read without allocating.
	pub fn snapshot(&self) -> Arc<Snapshot<T>> {
		self.snap.load_full()
	}

	/// Executes a closure while the snapshot guard is alive.
	pub fn with_snapshot<R>(&self, f: impl FnOnce(&Snapshot<T>) -> R) -> R {
		let snap = self.snap.load();
		f(&snap)
	}
}

/// KeyStore over Snapshot for shared insertion logic.
struct SnapshotStore<'a, T: RegistryEntry + 'static> {
	snap: &'a mut Snapshot<T>,
}

impl<T: RegistryEntry + 'static> KeyStore<T> for SnapshotStore<'_, T> {
	fn get_id_owner(&self, id: &str) -> Option<&'static T> {
		self.snap.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static T> {
		self.snap.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static T) {
		self.snap.by_key.insert(key, def);
	}

	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T> {
		match self.snap.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn set_id_owner(&mut self, id: &'static str, def: &'static T) {
		self.snap.by_id.insert(id, def);
	}

	fn evict_def(&mut self, def: &'static T) {
		self.snap.by_key.retain(|_, &mut v| !std::ptr::eq(v, def));
	}

	fn push_collision(&mut self, c: Collision) {
		self.snap.collisions.push(c);
	}
}
