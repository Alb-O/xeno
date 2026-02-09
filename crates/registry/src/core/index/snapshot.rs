//! Snapshot and pinning handle implementations.
//!
//! # Role
//!
//! This module provides the pure view types used to search and hold registry data.
//! It contains no mutation logic.
//!
//! # Invariants
//!
//! - `RegistryRef` must hold its source `Snapshot` alive while held (see `invariants::test_snapshot_liveness_across_swap`).

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::core::{Collision, DenseId, FrozenInterner, Party, Symbol};

/// Single source of truth for registry lookups.
pub struct Snapshot<T, Id: DenseId>
where
	T: super::RuntimeEntry,
{
	pub table: Arc<[Arc<T>]>,
	/// Stage A: Canonical ID lookup.
	pub(crate) by_id: Arc<FxHashMap<Symbol, Id>>,
	/// Stage B: Primary name lookup.
	pub(crate) by_name: Arc<FxHashMap<Symbol, Id>>,
	/// Stage C: Secondary key lookup.
	pub(crate) by_key: Arc<FxHashMap<Symbol, Id>>,
	pub interner: FrozenInterner,
	pub key_pool: Arc<[Symbol]>,
	pub collisions: Arc<[Collision]>,
	pub parties: Arc<[Party]>,
	/// Next ordinal for monotonic runtime assignment.
	pub next_ordinal: u32,
}

impl<T, Id: DenseId> Clone for Snapshot<T, Id>
where
	T: super::RuntimeEntry,
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
			next_ordinal: self.next_ordinal,
		}
	}
}

impl<T, Id: DenseId> Snapshot<T, Id>
where
	T: super::RuntimeEntry,
{
	/// Creates a new snapshot from a builtin index.
	pub(super) fn from_builtins(b: &super::types::RegistryIndex<T, Id>) -> Self {
		let next_ordinal = b
			.parties
			.iter()
			.map(|p| p.ordinal)
			.max()
			.unwrap_or(0)
			.saturating_add(1);
		Self {
			table: b.table.clone(),
			by_id: b.by_id.clone(),
			by_name: b.by_name.clone(),
			by_key: b.by_key.clone(),
			interner: b.interner.clone(),
			key_pool: b.key_pool.clone(),
			collisions: b.collisions.clone(),
			parties: b.parties.clone(),
			next_ordinal,
		}
	}
}

/// Snapshot-pinning guard that provides `&T` access to a registry definition.
pub struct RegistryRef<T, Id: DenseId>
where
	T: super::RuntimeEntry,
{
	pub(crate) snap: Arc<Snapshot<T, Id>>,
	pub(crate) id: Id,
}

impl<T, Id: DenseId> Clone for RegistryRef<T, Id>
where
	T: super::RuntimeEntry,
{
	fn clone(&self) -> Self {
		Self {
			snap: self.snap.clone(),
			id: self.id,
		}
	}
}

impl<T, Id: DenseId> std::fmt::Debug for RegistryRef<T, Id>
where
	T: super::RuntimeEntry,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RegistryRef")
			.field("id", &self.id)
			.field("name", &self.name_str())
			.finish()
	}
}

impl<T, Id: DenseId> RegistryRef<T, Id>
where
	T: super::RuntimeEntry,
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

	/// Returns an iterator over resolved secondary key strings.
	pub fn keys_resolved(&self) -> Vec<&str> {
		let meta = self.meta();
		let start = meta.keys.start as usize;
		let end = start + meta.keys.len as usize;
		self.snap.key_pool[start..end]
			.iter()
			.map(|&sym| self.snap.interner.resolve(sym))
			.collect()
	}
}

impl<T, Id: DenseId> std::ops::Deref for RegistryRef<T, Id>
where
	T: super::RuntimeEntry,
{
	type Target = T;

	fn deref(&self) -> &T {
		&self.snap.table[self.id.as_u32() as usize]
	}
}

/// Lightweight guard for efficient iteration without per-item Arc clones.
pub struct SnapshotGuard<T, Id: DenseId>
where
	T: super::RuntimeEntry,
{
	pub(crate) snap: Arc<Snapshot<T, Id>>,
}

impl<T, Id: DenseId> SnapshotGuard<T, Id>
where
	T: super::RuntimeEntry,
{
	/// Returns an iterator over all entries in the snapshot.
	pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
		self.snap.table.iter().map(|arc| arc.as_ref())
	}

	/// Returns an iterator over (Id, &T) pairs.
	pub fn iter_items(&self) -> impl Iterator<Item = (Id, &T)> + '_ {
		self.snap.table.iter().enumerate().map(|(idx, arc)| {
			(
				Id::from_u32(super::u32_index(idx, "snapshot_iter")),
				arc.as_ref(),
			)
		})
	}

	/// Returns an iterator over [`RegistryRef`] handles.
	pub fn iter_refs(self) -> impl Iterator<Item = RegistryRef<T, Id>> {
		let snap = self.snap;
		let len = snap.table.len();
		(0..len).map(move |idx| RegistryRef {
			snap: snap.clone(),
			id: Id::from_u32(super::u32_index(idx, "snapshot_iter_refs")),
		})
	}

	/// Returns the number of entries.
	pub fn len(&self) -> usize {
		self.snap.table.len()
	}

	/// Returns true if empty.
	pub fn is_empty(&self) -> bool {
		self.snap.table.is_empty()
	}
}
