use crate::{Collision, RegistryEntry};

pub(super) type Map<K, V> = rustc_hash::FxHashMap<K, V>;

/// Indexed collection of registry definitions with O(1) lookup.
pub struct RegistryIndex<T: RegistryEntry + 'static> {
	pub(crate) by_id: Map<&'static str, &'static T>,
	pub(crate) by_key: Map<&'static str, &'static T>,
	pub(crate) items_all: Vec<&'static T>,
	pub(crate) items_effective: Vec<&'static T>,
	pub(crate) collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> Clone for RegistryIndex<T> {
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

impl<T: RegistryEntry + 'static> RegistryIndex<T> {
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

	/// Returns all definitions submitted to the builder (including shadowed).
	#[inline]
	pub fn items_all(&self) -> &[&'static T] {
		&self.items_all
	}

	/// Returns effective definitions: unique set reachable via indices.
	#[inline]
	pub fn items(&self) -> &[&'static T] {
		&self.items_effective
	}

	/// Returns recorded collisions for diagnostics.
	#[inline]
	pub fn collisions(&self) -> &[Collision] {
		&self.collisions
	}

	/// Returns the number of effective definitions (not keys, not shadowed).
	#[inline]
	pub fn len(&self) -> usize {
		self.items_effective.len()
	}

	/// Returns true if the index contains no definitions.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.items_effective.is_empty()
	}

	/// Returns an iterator over effective definitions.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = &'static T> + '_ {
		self.items_effective.iter().copied()
	}
}
