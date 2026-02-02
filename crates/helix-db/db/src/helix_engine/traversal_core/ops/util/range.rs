use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;

pub struct Range<I> {
	iter: I,
	curr_idx: usize,
	start: usize,
	end: usize,
}

// implementing iterator for Range
impl<'arena, I> Iterator for Range<I>
where
	I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
{
	type Item = I::Item;

	fn next(&mut self) -> Option<Self::Item> {
		// skips to start
		while self.curr_idx < self.start {
			match self.iter.next() {
				Some(_) => self.curr_idx += 1,
				None => return None, // out of items
			}
		}

		// return between start and end
		if self.curr_idx < self.end {
			match self.iter.next() {
				Some(item) => {
					self.curr_idx += 1;
					Some(item)
				}
				None => None,
			}
		} else {
			// all consumed
			None
		}
	}
}

pub trait RangeAdapter<'db, 'arena, 'txn>: Iterator {
	/// Range returns a slice of the current step between two points
	///
	/// # Arguments
	///
	/// * `start` - The starting index
	/// * `end` - The ending index
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use bumpalo::Bump;
	/// # use helix_db::helix_engine::storage_core::HelixGraphStorage;
	/// # use helix_db::helix_engine::storage_core::version_info::VersionInfo;
	/// # use helix_db::helix_engine::traversal_core::config::Config;
	/// # use helix_db::helix_engine::traversal_core::ops::g::G;
	/// # use helix_db::helix_engine::traversal_core::ops::util::range::RangeAdapter;
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// # let path = std::env::temp_dir().join("xeno-docs-range");
	/// # let storage = HelixGraphStorage::new(
	/// #     path.to_str().unwrap(),
	/// #     Config::new(16, 128, 768, 1, false, false, None, None, None),
	/// #     VersionInfo::default(),
	/// # )?;
	/// # let arena = Bump::new();
	/// # let txn = storage.graph_env.read_txn()?;
	/// let traversal = G::new(&storage, &txn, &arena).range(0, 10);
	/// # Ok(())
	/// # }
	/// ```
	fn range<N, K>(
		self,
		start: N,
		end: K,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		Self: Sized + Iterator,
		N: TryInto<usize>,
		K: TryInto<usize>,
		N::Error: std::fmt::Debug,
		K::Error: std::fmt::Debug;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	RangeAdapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline(always)]
	fn range<N, K>(
		self,
		start: N,
		end: K,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>
	where
		Self: Sized + Iterator,
		N: TryInto<usize>,
		K: TryInto<usize>,
		N::Error: std::fmt::Debug,
		K::Error: std::fmt::Debug,
	{
		{
			let start_usize = start
				.try_into()
				.expect("Start index must be non-negative and fit in usize");
			let end_usize = end
				.try_into()
				.expect("End index must be non-negative and fit in usize");

			RoTraversalIterator {
				storage: self.storage,
				arena: self.arena,
				txn: self.txn,
				inner: Range {
					iter: self.inner,
					curr_idx: 0,
					start: start_usize,
					end: end_usize,
				},
			}
		}
	}
}
