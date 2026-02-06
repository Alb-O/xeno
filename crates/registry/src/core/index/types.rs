use std::sync::Arc;

use crate::{Collision, RegistryEntry};

pub(super) type Map<K, V> = rustc_hash::FxHashMap<K, V>;

/// Reference to a registry definition, either builtin or owned.
#[derive(Debug)]
pub enum DefRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	/// Compile-time definition.
	Builtin(&'static T),
	/// Runtime-registered definition.
	Owned(Arc<T>),
}

/// Alias for compatibility during refactor.
pub type DefPtr<T> = DefRef<T>;

impl<T> Clone for DefRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn clone(&self) -> Self {
		match self {
			Self::Builtin(v) => Self::Builtin(v),
			Self::Owned(v) => Self::Owned(v.clone()),
		}
	}
}

impl<T> DefRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	/// Returns a reference to the underlying definition.
	#[inline]
	pub fn as_entry(&self) -> &T {
		match self {
			Self::Builtin(v) => v,
			Self::Owned(v) => v,
		}
	}

	/// Returns a reference to the underlying definition.
	///
	/// # Safety
	///
	/// This is safe because the reference is kept alive by the variant itself.
	/// This method is kept for compatibility with existing tests/code and marked
	/// as safe internally.
	#[inline]
	pub unsafe fn as_ref(&self) -> &T {
		self.as_entry()
	}

	/// Returns true if both references point to the same definition instance.
	#[inline]
	pub fn ptr_eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Builtin(a), Self::Builtin(b)) => std::ptr::eq(*a, *b),
			(Self::Owned(a), Self::Owned(b)) => Arc::ptr_eq(a, b),
			_ => false,
		}
	}

	/// Returns a raw pointer for identity checks (HashSet dedup).
	#[inline]
	pub fn identity(&self) -> *const T {
		match self {
			Self::Builtin(v) => *v as *const T,
			Self::Owned(v) => Arc::as_ptr(v),
		}
	}
}

impl<T> PartialEq for DefRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn eq(&self, other: &Self) -> bool {
		self.ptr_eq(other)
	}
}

impl<T> Eq for DefRef<T> where T: RegistryEntry + Send + Sync + 'static {}

impl<T> std::hash::Hash for DefRef<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.identity().hash(state);
	}
}

/// Indexed collection of registry definitions with O(1) lookup.
pub struct RegistryIndex<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	pub(crate) by_id: Map<Box<str>, DefRef<T>>,
	pub(crate) by_key: Map<Box<str>, DefRef<T>>,
	pub(crate) items_all: Vec<DefRef<T>>,
	pub(crate) items_effective: Vec<DefRef<T>>,
	pub(crate) id_order: Vec<Box<str>>,
	pub(crate) collisions: Vec<Collision>,
}

impl<T> Clone for RegistryIndex<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn clone(&self) -> Self {
		Self {
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			items_all: self.items_all.clone(),
			items_effective: self.items_effective.clone(),
			id_order: self.id_order.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T> RegistryIndex<T>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&T> {
		self.by_id
			.get(key)
			.or_else(|| self.by_key.get(key))
			.map(|p| p.as_entry())
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<&T> {
		self.by_id.get(id).map(|p| p.as_entry())
	}

	/// Returns all definitions submitted to the builder (including shadowed).
	#[inline]
	pub fn items_all(&self) -> &[DefRef<T>] {
		&self.items_all
	}

	/// Returns effective definitions: unique set reachable via indices.
	#[inline]
	pub fn items(&self) -> &[DefRef<T>] {
		&self.items_effective
	}

	/// Returns an iterator over effective definitions in stable order.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
		self.id_order
			.iter()
			.filter_map(|id| self.by_id.get(id))
			.map(|p| p.as_entry())
	}

	/// Returns recorded collisions for diagnostics.
	#[inline]
	pub fn collisions(&self) -> &[Collision] {
		&self.collisions
	}

	/// Returns the number of effective definitions.
	#[inline]
	pub fn len(&self) -> usize {
		self.id_order.len()
	}

	/// Returns true if the index contains no definitions.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.id_order.is_empty()
	}
}
