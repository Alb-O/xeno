use std::cmp::Ordering;
use std::collections::HashMap;

use super::collision::{
	ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore, insert_typed_key,
};
use super::mod_types::RegistryIndex;
use crate::RegistryEntry;

/// Runtime overlay for registry extensions.
pub(super) struct RuntimeExtras<T: RegistryEntry + 'static> {
	pub items: Vec<&'static T>,
	pub by_id: HashMap<&'static str, &'static T>,
	pub by_key: HashMap<&'static str, &'static T>,
	pub collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> Clone for RuntimeExtras<T> {
	fn clone(&self) -> Self {
		Self {
			items: self.items.clone(),
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T: RegistryEntry + 'static> Default for RuntimeExtras<T> {
	fn default() -> Self {
		Self {
			items: Vec::new(),
			by_id: HashMap::new(),
			by_key: HashMap::new(),
			collisions: Vec::new(),
		}
	}
}

/// Registry wrapper for runtime-extensible registries.
pub struct RuntimeRegistry<T: RegistryEntry + 'static> {
	pub(super) label: &'static str,
	pub(super) builtins: RegistryIndex<T>,
	pub(super) extras: std::sync::RwLock<RuntimeExtras<T>>,
	pub(super) policy: DuplicatePolicy,
}

macro_rules! poison_policy {
	($lock:expr, $method:ident) => {
		if cfg!(any(test, debug_assertions)) {
			$lock.$method().unwrap_or_else(|e| e.into_inner())
		} else {
			$lock.$method().expect("registry lock poisoned")
		}
	};
}

impl<T: RegistryEntry + 'static> RuntimeRegistry<T> {
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T>) -> Self {
		Self {
			label,
			builtins,
			extras: std::sync::RwLock::new(RuntimeExtras::default()),
			policy: DuplicatePolicy::for_build(),
		}
	}

	/// Creates a new runtime registry with a custom duplicate policy.
	pub fn with_policy(
		label: &'static str,
		builtins: RegistryIndex<T>,
		policy: DuplicatePolicy,
	) -> Self {
		Self {
			label,
			builtins,
			extras: std::sync::RwLock::new(RuntimeExtras::default()),
			policy,
		}
	}

	/// Looks up a definition by ID, name, or alias.
	pub fn get(&self, key: &str) -> Option<&'static T> {
		let extras = poison_policy!(self.extras, read);

		extras
			.by_id
			.get(key)
			.copied()
			.or_else(|| self.builtins.get_by_id(key))
			.or_else(|| extras.by_key.get(key).copied())
			.or_else(|| self.builtins.get(key))
	}

	/// Returns the definition for a given ID, if it exists.
	pub fn get_by_id(&self, id: &str) -> Option<&'static T> {
		let extras = poison_policy!(self.extras, read);
		extras
			.by_id
			.get(id)
			.copied()
			.or_else(|| self.builtins.get_by_id(id))
	}

	/// Registers a definition at runtime.
	pub fn register(&self, def: &'static T) -> bool {
		if self
			.builtins
			.items_all()
			.iter()
			.any(|&b| std::ptr::eq(b, def))
		{
			return false;
		}

		let mut extras_guard = poison_policy!(self.extras, write);

		if extras_guard.items.iter().any(|&e| std::ptr::eq(e, def)) {
			return false;
		}

		let mut extras = (*extras_guard).clone();
		let meta = def.meta();
		let choose_winner = self.make_choose_winner();
		let mut store = RuntimeStore {
			builtins: &self.builtins,
			extras: &mut extras,
		};

		if let Err(e) = insert_typed_key(
			&mut store,
			self.label,
			choose_winner,
			KeyKind::Id,
			meta.id,
			def,
		) {
			panic!("runtime registry {}: {}", self.label, e);
		}

		if let Err(e) = insert_typed_key(
			&mut store,
			self.label,
			choose_winner,
			KeyKind::Name,
			meta.name,
			def,
		) {
			panic!("runtime registry {}: {}", self.label, e);
		}

		for &alias in meta.aliases {
			if let Err(e) = insert_typed_key(
				&mut store,
				self.label,
				choose_winner,
				KeyKind::Alias,
				alias,
				def,
			) {
				panic!("runtime registry {}: {}", self.label, e);
			}
		}

		extras.items.push(def);
		*extras_guard = extras;
		true
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
		self.builtins.len() + poison_policy!(self.extras, read).items.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.builtins.is_empty() && poison_policy!(self.extras, read).items.is_empty()
	}

	/// Returns all definitions (builtins followed by extras).
	pub fn all(&self) -> Vec<&'static T> {
		let mut items: Vec<_> = self.builtins.items().to_vec();
		items.extend(poison_policy!(self.extras, read).items.iter().copied());
		items
	}

	/// Returns definitions added at runtime.
	pub fn extras_items(&self) -> Vec<&'static T> {
		poison_policy!(self.extras, read).items.clone()
	}

	/// Returns the underlying builtins index.
	pub fn builtins(&self) -> &RegistryIndex<T> {
		&self.builtins
	}

	/// Returns an iterator over builtin definitions only.
	pub fn iter(&self) -> impl Iterator<Item = &'static T> + '_ {
		self.builtins.iter()
	}

	/// Returns the builtin items slice.
	pub fn items(&self) -> &[&'static T] {
		self.builtins.items()
	}

	/// Returns all recorded collisions (builtins + runtime).
	pub fn collisions(&self) -> Vec<Collision> {
		let mut collisions = self.builtins.collisions().to_vec();
		collisions.extend(poison_policy!(self.extras, read).collisions.iter().cloned());
		collisions
	}
}

/// Layered [`KeyStore`] for runtime insertion: checks builtins first, then extras.
struct RuntimeStore<'a, T: RegistryEntry + 'static> {
	builtins: &'a RegistryIndex<T>,
	extras: &'a mut RuntimeExtras<T>,
}

impl<T: RegistryEntry + 'static> KeyStore<T> for RuntimeStore<'_, T> {
	fn get_id_owner(&self, id: &str) -> Option<&'static T> {
		self.builtins
			.get_by_id(id)
			.or_else(|| self.extras.by_id.get(id).copied())
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static T> {
		self.extras
			.by_key
			.get(key)
			.copied()
			.or_else(|| self.builtins.get(key))
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static T) {
		self.extras.by_key.insert(key, def);
	}

	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T> {
		if let Some(builtin) = self.builtins.get_by_id(id) {
			return Some(builtin);
		}
		match self.extras.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.extras.collisions.push(c);
	}
}
