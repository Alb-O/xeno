use std::cmp::Ordering;
use std::collections::HashMap;

use super::collision::{ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore};
use super::insert::insert_typed_key;
use super::types::RegistryIndex;
use crate::RegistryEntry;

/// Builder for constructing a [`RegistryIndex`].
pub struct RegistryBuilder<T: RegistryEntry + 'static> {
	label: &'static str,
	defs: Vec<&'static T>,
	include_id: bool,
	include_name: bool,
	include_aliases: bool,
	policy: DuplicatePolicy,
}

impl<T: RegistryEntry + 'static> RegistryBuilder<T> {
	/// Creates a new builder with the given label for error messages.
	pub fn new(label: &'static str) -> Self {
		Self {
			label,
			defs: Vec::new(),
			include_id: true,
			include_name: true,
			include_aliases: true,
			policy: DuplicatePolicy::for_build(),
		}
	}

	/// Sets whether to index definitions by their id.
	pub fn include_id(mut self, on: bool) -> Self {
		self.include_id = on;
		self
	}

	/// Sets whether to index definitions by their name.
	pub fn include_name(mut self, on: bool) -> Self {
		self.include_name = on;
		self
	}

	/// Sets whether to index definitions by their aliases.
	pub fn include_aliases(mut self, on: bool) -> Self {
		self.include_aliases = on;
		self
	}

	/// Sets the duplicate key handling policy.
	pub fn duplicate_policy(mut self, policy: DuplicatePolicy) -> Self {
		self.policy = policy;
		self
	}

	/// Adds a single definition to the builder.
	pub fn push(&mut self, def: &'static T) {
		self.defs.push(def);
	}

	/// Adds multiple definitions to the builder.
	pub fn extend<I: IntoIterator<Item = &'static T>>(&mut self, defs: I) {
		self.defs.extend(defs);
	}

	/// Sorts definitions using the provided comparison function.
	pub fn sort_by<F: FnMut(&&'static T, &&'static T) -> Ordering>(mut self, cmp: F) -> Self {
		self.defs.sort_by(cmp);
		self
	}

	/// Sorts definitions using the global total order.
	pub fn sort_default(mut self) -> Self {
		self.defs.sort_by(|a, b| b.total_order_cmp(a));
		self
	}

	/// Builds the index with two-pass insertion and invariant enforcement.
	pub fn build(self) -> RegistryIndex<T> {
		let policy = self.policy;
		let label = self.label;
		let include_id = self.include_id;
		let include_name = self.include_name;
		let include_aliases = self.include_aliases;

		let mut defs = self.defs; // Move out of self
		let mut seen: std::collections::HashSet<*const T> =
			std::collections::HashSet::with_capacity(defs.len());
		defs.retain(|d| seen.insert(*d as *const T));

		let mut store = BuildStore::<T> {
			by_id: HashMap::with_capacity(defs.len()),
			by_key: HashMap::with_capacity(defs.len() * 2),
			collisions: Vec::new(),
		};

		let choose_winner = Self::make_choose_winner(policy);

		if include_id {
			for &def in &defs {
				if let Err(e) = insert_typed_key(
					&mut store,
					label,
					choose_winner,
					KeyKind::Id,
					def.meta().id,
					def,
				) {
					panic!("registry {}: {}", label, e);
				}
			}
		}

		for &def in &defs {
			let meta = def.meta();

			if include_name
				&& let Err(e) = insert_typed_key(
					&mut store,
					label,
					choose_winner,
					KeyKind::Name,
					meta.name,
					def,
				) {
				panic!("registry {}: {}", label, e);
			}

			if include_aliases {
				for &alias in meta.aliases {
					if let Err(e) = insert_typed_key(
						&mut store,
						label,
						choose_winner,
						KeyKind::Alias,
						alias,
						def,
					) {
						panic!("registry {}: {}", label, e);
					}
				}
			}
		}

		let mut effective_set: std::collections::HashSet<*const T> =
			std::collections::HashSet::with_capacity(defs.len());
		for &def in store.by_id.values() {
			effective_set.insert(def as *const T);
		}
		for &def in store.by_key.values() {
			effective_set.insert(def as *const T);
		}

		let items_effective: Vec<&'static T> = defs
			.iter()
			.copied()
			.filter(|&d| effective_set.contains(&(d as *const T)))
			.collect();

		RegistryIndex {
			by_id: store.by_id,
			by_key: store.by_key,
			items_all: defs,
			items_effective,
			collisions: store.collisions,
		}
	}

	/// Creates a winner selection function based on the policy.
	fn make_choose_winner(policy: DuplicatePolicy) -> ChooseWinner<T> {
		match policy {
			DuplicatePolicy::Panic => |kind, key, existing, new| {
				panic!(
					"duplicate registry key: kind={} key={:?} existing_id={} new_id={}",
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
}

/// Temporary storage for build-time key insertion.
struct BuildStore<T: RegistryEntry + 'static> {
	by_id: HashMap<&'static str, &'static T>,
	by_key: HashMap<&'static str, &'static T>,
	collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> KeyStore<T> for BuildStore<T> {
	fn get_id_owner(&self, id: &str) -> Option<&'static T> {
		self.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static T> {
		self.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static T) {
		self.by_key.insert(key, def);
	}

	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T> {
		match self.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
	}

	fn push_collision(&mut self, c: Collision) {
		self.collisions.push(c);
	}
}
