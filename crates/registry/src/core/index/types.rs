use crate::{Collision, RegistryEntry};

pub(super) type Map<K, V> = rustc_hash::FxHashMap<K, V>;

#[repr(transparent)]
pub struct DefPtr<T: ?Sized>(*const T);

impl<T: ?Sized> Clone for DefPtr<T> {
	#[inline]
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized> Copy for DefPtr<T> {}

impl<T: ?Sized> PartialEq for DefPtr<T> {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		std::ptr::addr_eq(self.0, other.0)
	}
}

impl<T: ?Sized> Eq for DefPtr<T> {}

impl<T: ?Sized> std::hash::Hash for DefPtr<T> {
	#[inline]
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.hash(state);
	}
}

impl<T: ?Sized> DefPtr<T> {
	#[inline]
	pub fn from_ref(v: &T) -> Self {
		Self(v as *const T)
	}

	#[inline]
	pub fn ptr_eq(self, other: Self) -> bool {
		std::ptr::addr_eq(self.0, other.0)
	}

	/// # Safety
	/// The caller must ensure the pointed-to `T` outlives the returned reference.
	#[inline]
	pub unsafe fn as_ref<'a>(self) -> &'a T {
		unsafe { &*self.0 }
	}
}

impl<T: ?Sized> std::fmt::Debug for DefPtr<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("DefPtr").field(&self.0).finish()
	}
}

// Needed if Snapshot<T> is shared across threads (it is via ArcSwap).
unsafe impl<T: Sync + ?Sized> Send for DefPtr<T> {}
unsafe impl<T: Sync + ?Sized> Sync for DefPtr<T> {}

/// Indexed collection of registry definitions with O(1) lookup.
pub struct RegistryIndex<T: RegistryEntry + Send + Sync + 'static> {
	pub(crate) by_id: Map<Box<str>, DefPtr<T>>,
	pub(crate) by_key: Map<Box<str>, DefPtr<T>>,
	pub(crate) items_all: Vec<DefPtr<T>>,
	pub(crate) items_effective: Vec<DefPtr<T>>,
	pub(crate) collisions: Vec<Collision>,
}

impl<T: RegistryEntry + Send + Sync + 'static> Clone for RegistryIndex<T> {
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

impl<T: RegistryEntry + Send + Sync + 'static> RegistryIndex<T> {
	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&T> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
			.map(|p| unsafe { p.as_ref() })
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<&T> {
		self.by_id.get(id).copied().map(|p| unsafe { p.as_ref() })
	}

	/// Returns all definitions submitted to the builder (including shadowed).
	#[inline]
	pub fn items_all(&self) -> &[DefPtr<T>] {
		&self.items_all
	}

	/// Returns effective definitions: unique set reachable via indices.
	#[inline]
	pub fn items(&self) -> &[DefPtr<T>] {
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
	pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
		self.items_effective
			.iter()
			.copied()
			.map(|p| unsafe { p.as_ref() })
	}
}
