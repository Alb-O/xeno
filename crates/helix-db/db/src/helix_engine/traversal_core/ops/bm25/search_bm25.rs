use crate::helix_engine::bm25::bm25::BM25;
use crate::helix_engine::traversal_core::LMDB_STRING_HEADER_LENGTH;
use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, StorageError, TraversalError};
use crate::utils::items::Node;

pub trait SearchBM25Adapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	fn search_bm25<K>(
		self,
		label: &'arena str,
		query: &str,
		k: K,
	) -> Result<
		RoTraversalIterator<
			'db,
			'arena,
			'txn,
			impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
		>,
		EngineError,
	>
	where
		K: TryInto<usize>,
		K::Error: std::fmt::Debug;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	SearchBM25Adapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	fn search_bm25<K>(
		self,
		label: &'arena str,
		query: &str,
		k: K,
	) -> Result<
		RoTraversalIterator<
			'db,
			'arena,
			'txn,
			impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
		>,
		EngineError,
	>
	where
		K: TryInto<usize>,
		K::Error: std::fmt::Debug,
	{
		let results = match self.storage.bm25.as_ref() {
			Some(s) => s.search(self.txn, query, k.try_into().unwrap(), self.arena)?,
			None => {
				return Err(TraversalError::Message("BM25 not enabled!".to_string()).into());
			}
		};

		let label_as_bytes = label.as_bytes();
		let iter = results.into_iter().filter_map(move |(id, score)| {
            if let Ok(Some(value)) = self.storage.nodes_db.get(self.txn, &id) {
            assert!(
                value.len() >= LMDB_STRING_HEADER_LENGTH,
                "value length does not contain header which means the `label` field was missing from the node on insertion"
            );
            let length_of_label_in_lmdb =
                u64::from_le_bytes(value[..LMDB_STRING_HEADER_LENGTH].try_into().unwrap()) as usize;

            if length_of_label_in_lmdb != label.len() {
                return None;
            }

            assert!(
                value.len() >= length_of_label_in_lmdb + LMDB_STRING_HEADER_LENGTH,
                "value length is not at least the header length plus the label length meaning there has been a corruption on node insertion"
            );
            let label_in_lmdb = &value[LMDB_STRING_HEADER_LENGTH
                ..LMDB_STRING_HEADER_LENGTH + length_of_label_in_lmdb];

            if label_in_lmdb == label_as_bytes {
                match Node::<'arena>::from_bytes(id, value, self.arena) {
                    Ok(node) => {
                        return Some(Ok(TraversalValue::NodeWithScore { node, score: score as f64 }));
                    }
					Err(e) => {
						tracing::warn!(?e, node_id = %id, "error decoding node");
						return Some(Err(StorageError::Conversion(e.to_string()).into()));
					}
				}
            } else {
                return None;
            }
            }
            None
        });

		Ok(RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: iter,
		})
	}
}
