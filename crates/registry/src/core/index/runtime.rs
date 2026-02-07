//! Registry definition indexing and runtime extension system.

use std::sync::Arc;

use arc_swap::ArcSwap;
use rustc_hash::FxHashMap;

use super::types::RegistryIndex;
use crate::core::{Collision, DenseId, FrozenInterner, RegistryEntry, Symbol};

/// Marker trait for types that can be stored in a runtime registry.
pub trait RuntimeEntry: RegistryEntry + Send + Sync + 'static {}
impl<T> RuntimeEntry for T where T: RegistryEntry + Send + Sync + 'static {}

/// Snapshot-pinning guard that provides `&T` access to a registry definition.
pub struct RegistryRef<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub(crate) snap: Arc<Snapshot<T, Id>>,
	pub(crate) id: Id,
}

impl<T, Id: DenseId> Clone for RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			id: self.id,
		}
	}
}

impl<T, Id: DenseId> RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	/// Returns the dense ID for this definition.
	pub fn dense_id(&self) -> Id {
		self.id
	}

	/// Resolves a symbol to its string representation using this ref's snapshot interner.
	pub fn resolve(&self, sym: crate::core::Symbol) -> &str {
		self.snap.interner.resolve(sym)
	}

	/// Returns the interned name as a string.
	pub fn name_str(&self) -> &str {
		self.resolve(self.name())
	}

	/// Returns the interned id as a string.
	pub fn id_str(&self) -> &str {
		self.resolve(self.id())
	}

	/// Returns the interned description as a string.
	pub fn description_str(&self) -> &str {
		self.resolve(self.description())
	}

	/// Returns an iterator over resolved alias strings.
	pub fn aliases_resolved(&self) -> Vec<&str> {
		let meta = self.meta();
		let start = meta.aliases.start as usize;
		let end = start + meta.aliases.len as usize;
		self.snap.alias_pool[start..end]
			.iter()
			.map(|&sym| self.snap.interner.resolve(sym))
			.collect()
	}
}

impl<T, Id: DenseId> std::ops::Deref for RegistryRef<T, Id>
where
	T: RuntimeEntry,
{
	type Target = T;

	fn deref(&self) -> &T {
		&self.snap.table[self.id.as_u32() as usize]
	}
}

/// Single source of truth for registry lookups.
pub struct Snapshot<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub table: Arc<[T]>,
	pub by_key: Arc<FxHashMap<Symbol, Id>>,
	pub interner: FrozenInterner,
	pub alias_pool: Arc<[Symbol]>,
	pub collisions: Arc<[Collision]>,
}

impl<T, Id: DenseId> Clone for Snapshot<T, Id>
where
	T: RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			table: self.table.clone(),
			by_key: self.by_key.clone(),
			interner: self.interner.clone(),
			alias_pool: self.alias_pool.clone(),
			collisions: self.collisions.clone(),
		}
	}
}

impl<T, Id: DenseId> Snapshot<T, Id>
where
	T: RuntimeEntry,
{
	/// Creates a new snapshot from a builtin index.
	fn from_builtins(b: &RegistryIndex<T, Id>) -> Self {
		Self {
			table: b.table.clone(),
			by_key: b.by_key.clone(),
			interner: b.interner.clone(),
			alias_pool: b.alias_pool.clone(),
			collisions: b.collisions.clone(),
		}
	}
}

/// Registry wrapper for runtime-extensible registries.
pub struct RuntimeRegistry<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub(super) label: &'static str,
	pub(super) builtins: RegistryIndex<T, Id>,
	pub(super) snap: ArcSwap<Snapshot<T, Id>>,
}

impl<T, Id: DenseId> RuntimeRegistry<T, Id>
where
	T: RuntimeEntry,
{
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T, Id>) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self {
			label,
			builtins,
			snap: ArcSwap::from_pointee(snap),
		}
	}

	/// Looks up a definition by ID, name, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let sym = snap.interner.get(key)?;
		let id = *snap.by_key.get(&sym)?;
		Some(RegistryRef { snap, id })
	}

	/// Looks up a definition by its interned symbol.
	#[inline]
	pub fn get_sym(&self, sym: Symbol) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let id = *snap.by_key.get(&sym)?;
		Some(RegistryRef { snap, id })
	}

	/// Returns all effective definitions.
	pub fn all(&self) -> Vec<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		let mut refs = Vec::with_capacity(snap.table.len());
		for i in 0..snap.table.len() {
			refs.push(RegistryRef {
				snap: snap.clone(),
				id: Id::from_u32(i as u32),
			});
		}
		refs
	}

	/// Looks up a definition by its dense ID.
	#[inline]
	pub fn get_by_id(&self, id: Id) -> Option<RegistryRef<T, Id>> {
		let snap = self.snap.load_full();
		if (id.as_u32() as usize) < snap.table.len() {
			Some(RegistryRef { snap, id })
		} else {
			None
		}
	}

	/// Returns a snapshot guard for direct interner access.
	pub fn snapshot(&self) -> Arc<Snapshot<T, Id>> {
		self.snap.load_full()
	}

	/// Registers a new definition at runtime (stub - not yet implemented).
	///
	/// Returns `false` because runtime registration requires interner extension
	/// which is not yet supported with the frozen interner architecture.
	pub fn register<In>(&self, _def: &'static In) -> bool
	where
		In: super::BuildEntry<T>,
	{
		false
	}

	/// Returns an iterator over all definitions as `RegistryRef`s.
	pub fn iter(&self) -> Vec<RegistryRef<T, Id>> {
		self.all()
	}

	/// Returns the number of effective definitions.
	pub fn len(&self) -> usize {
		self.snap.load().table.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}
