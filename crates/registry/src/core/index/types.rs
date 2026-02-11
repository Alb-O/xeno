use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::core::{Collision, DenseId, FrozenInterner, Party, RegistryEntry, Symbol};

pub(super) type Map<K, V> = FxHashMap<K, V>;

/// Indexed collection of registry definitions with O(1) lookup.
pub struct RegistryIndex<T, Id: DenseId>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	pub(crate) table: Arc<[Arc<T>]>,
	pub(crate) by_id: Arc<Map<Symbol, Id>>,
	pub(crate) by_name: Arc<Map<Symbol, Id>>,
	pub(crate) by_key: Arc<Map<Symbol, Id>>,
	pub(crate) interner: FrozenInterner,
	pub(crate) key_pool: Arc<[Symbol]>,
	pub(crate) collisions: Arc<[Collision]>,
	pub(crate) parties: Arc<[Party]>,
}

impl<T, Id: DenseId> Clone for RegistryIndex<T, Id>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	fn clone(&self) -> Self {
		Self {
			table: self.table.clone(),
			by_id: self.by_id.clone(),
			by_name: self.by_name.clone(),
			by_key: self.by_key.clone(),
			interner: self.interner.clone(),
			key_pool: self.key_pool.clone(),
			collisions: self.collisions.clone(),
			parties: self.parties.clone(),
		}
	}
}

impl<T, Id: DenseId> RegistryIndex<T, Id>
where
	T: RegistryEntry + Send + Sync + 'static,
{
	/// Looks up a definition by ID, name, or secondary key.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&T> {
		let sym = self.interner.get(key)?;
		self.get_sym(sym)
	}

	/// Looks up a definition by its interned symbol.
	///
	/// Uses 3-stage fallback: canonical ID → primary name → secondary keys.
	#[inline]
	pub fn get_sym(&self, sym: Symbol) -> Option<&T> {
		let id = self.by_id.get(&sym).or_else(|| self.by_name.get(&sym)).or_else(|| self.by_key.get(&sym))?;
		Some(&self.table[id.as_u32() as usize])
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: Id) -> Option<&T> {
		self.table.get(id.as_u32() as usize).map(|arc| arc.as_ref())
	}

	/// Returns all effective definitions in stable order.
	#[inline]
	pub fn items(&self) -> &[Arc<T>] {
		&self.table
	}

	/// Returns an iterator over effective definitions in stable order.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
		self.table.iter().map(|arc| arc.as_ref())
	}

	/// Returns recorded collisions for diagnostics.
	#[inline]
	pub fn collisions(&self) -> &[Collision] {
		&self.collisions
	}

	/// Returns the number of effective definitions.
	#[inline]
	pub fn len(&self) -> usize {
		self.table.len()
	}

	/// Returns true if the index contains no definitions.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.table.is_empty()
	}
}
