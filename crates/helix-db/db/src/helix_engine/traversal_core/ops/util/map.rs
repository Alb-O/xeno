use heed3::RoTxn;

use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;

pub struct Map<'db, 'txn, I, F> {
	iter: I,
	txn: &'txn RoTxn<'db>,
	f: F,
}

// implementing iterator for filter ref
impl<'db, 'arena, 'txn, I, F> Iterator for Map<'db, 'txn, I, F>
where
	I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	F: FnMut(TraversalValue<'arena>, &RoTxn<'db>) -> Result<TraversalValue<'arena>, EngineError>,
{
	type Item = I::Item;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(item) = self.iter.by_ref().next() {
			return match item {
				Ok(item) => Some((self.f)(item, self.txn)),
				Err(e) => return Some(Err(e)),
			};
		}
		None
	}
}

pub trait MapAdapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	/// MapTraversal maps the iterator by taking a reference
	/// to each item and a transaction.
	///
	/// # Arguments
	///
	/// * `f` - A function to map the iterator
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use bumpalo::Bump;
	/// # use helix_db::helix_engine::storage_core::HelixGraphStorage;
	/// # use helix_db::helix_engine::storage_core::version_info::VersionInfo;
	/// # use helix_db::helix_engine::traversal_core::config::Config;
	/// # use helix_db::helix_engine::traversal_core::ops::g::G;
	/// # use helix_db::helix_engine::traversal_core::ops::util::map::MapAdapter;
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// # let path = std::env::temp_dir().join("xeno-docs-map");
	/// # let storage = HelixGraphStorage::new(
	/// #     path.to_str().unwrap(),
	/// #     Config::new(16, 128, 768, 1, false, false, None, None, None),
	/// #     VersionInfo::default(),
	/// # )?;
	/// # let arena = Bump::new();
	/// # let txn = storage.graph_env.read_txn()?;
	/// let traversal = G::new(&storage, &txn, &arena).map_traversal(|item, _txn| Ok(item));
	/// # Ok(())
	/// # }
	/// ```
	fn map_traversal<F>(
		self,
		f: F,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: FnMut(
			TraversalValue<'arena>,
			&RoTxn<'db>,
		) -> Result<TraversalValue<'arena>, EngineError>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	MapAdapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline]
	fn map_traversal<F>(
		self,
		f: F,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		F: FnMut(
			TraversalValue<'arena>,
			&RoTxn<'db>,
		) -> Result<TraversalValue<'arena>, EngineError>,
	{
		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: Map {
				iter: self.inner,
				txn: self.txn,
				f,
			},
		}
	}
}
