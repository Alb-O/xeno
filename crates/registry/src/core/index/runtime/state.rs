use std::sync::Arc;

use super::*;

/// Marker trait for types that can be stored in a runtime registry.
pub trait RuntimeEntry: RegistryEntry + Send + Sync + 'static {}
impl<T> RuntimeEntry for T where T: RegistryEntry + Send + Sync + 'static {}

/// Immutable runtime registry view.
pub struct RuntimeRegistry<T, Id: DenseId>
where
	T: RuntimeEntry,
{
	pub(super) snap: Arc<Snapshot<T, Id>>,
}

impl<T, Id: DenseId> RuntimeRegistry<T, Id>
where
	T: RuntimeEntry,
{
	/// Creates an immutable runtime registry from builtins.
	pub fn new(_label: &'static str, builtins: RegistryIndex<T, Id>) -> Self {
		let snap = Snapshot::from_builtins(&builtins);
		Self { snap: Arc::new(snap) }
	}

	/// Looks up a definition by ID, name, or secondary key.
	///
	/// Uses 3-stage fallback: canonical ID → primary name → secondary keys.
	#[inline]
	pub fn get(&self, key: &str) -> Option<RegistryRef<T, Id>> {
		let snap = Arc::clone(&self.snap);
		let sym = snap.interner.get(key)?;
		self.get_sym_with_snap(snap, sym)
	}

	/// Looks up a definition by its interned symbol.
	///
	/// Uses 3-stage fallback: canonical ID → primary name → secondary keys.
	#[inline]
	pub fn get_sym(&self, sym: Symbol) -> Option<RegistryRef<T, Id>> {
		let snap = Arc::clone(&self.snap);
		self.get_sym_with_snap(snap, sym)
	}

	#[inline]
	fn get_sym_with_snap(&self, snap: Arc<Snapshot<T, Id>>, sym: Symbol) -> Option<RegistryRef<T, Id>> {
		let id = snap
			.by_id
			.get(&sym)
			.or_else(|| snap.by_name.get(&sym))
			.or_else(|| snap.by_key.get(&sym))
			.copied()?;
		Some(RegistryRef { snap, id })
	}

	/// Looks up a definition by its untyped key.
	pub fn get_key(&self, key: &crate::core::LookupKey<T, Id>) -> Option<RegistryRef<T, Id>> {
		match key {
			crate::core::LookupKey::Static(s) => self.get(s),
			crate::core::LookupKey::Ref(r) => Some(r.clone()),
		}
	}

	/// Returns a snapshot guard for efficient iteration.
	pub fn snapshot_guard(&self) -> SnapshotGuard<T, Id> {
		SnapshotGuard { snap: Arc::clone(&self.snap) }
	}

	/// Looks up a definition by its dense ID.
	#[inline]
	pub fn get_by_id(&self, id: Id) -> Option<RegistryRef<T, Id>> {
		let snap = Arc::clone(&self.snap);
		if (id.as_u32() as usize) < snap.table.len() {
			Some(RegistryRef { snap, id })
		} else {
			None
		}
	}

	/// Returns a snapshot guard for direct interner access.
	pub fn snapshot(&self) -> Arc<Snapshot<T, Id>> {
		Arc::clone(&self.snap)
	}

	/// Returns the number of effective definitions.
	pub fn len(&self) -> usize {
		self.snap.table.len()
	}

	/// Returns collision diagnostics captured for this domain.
	pub fn collisions(&self) -> &[crate::core::Collision] {
		&self.snap.collisions
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}
