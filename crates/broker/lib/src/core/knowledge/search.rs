//! Full-text search and ranked retrieval.

use bumpalo::Bump;
use helix_db::helix_engine::traversal_core::ops::bm25::search_bm25::SearchBM25Adapter;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::protocol::value::Value;
use xeno_broker_proto::types::KnowledgeHit;

use super::{KnowledgeCore, KnowledgeError};

const LABEL_CHUNK: &str = "Chunk";

impl KnowledgeCore {
	/// Executes a BM25 ranked search against the workspace index.
	pub fn search(&self, query: &str, limit: u32) -> Result<Vec<KnowledgeHit>, KnowledgeError> {
		let arena = Bump::new();
		let txn = self
			.storage
			.graph_env
			.read_txn()
			.map_err(helix_db::helix_engine::types::EngineError::from)?;

		let mut hits = Vec::new();
		let traversal =
			G::new(self.storage(), &txn, &arena).search_bm25(LABEL_CHUNK, query, limit as usize)?;

		for entry in traversal {
			let (tv, score) = match entry {
				Ok(TraversalValue::NodeWithScore { node, score }) => {
					(TraversalValue::Node(node), score)
				}
				Ok(tv) => (tv, 0.0),
				Err(err) => {
					tracing::warn!(error = %err, "search traversal entry error");
					continue;
				}
			};

			if let TraversalValue::Node(node) = tv {
				let uri = node
					.get_property("doc_uri")
					.and_then(|v| match v {
						Value::String(s) => Some(s.clone()),
						_ => None,
					})
					.unwrap_or_default();

				let preview = node
					.get_property("text")
					.and_then(|v| match v {
						Value::String(s) => Some(s.clone()),
						_ => None,
					})
					.unwrap_or_default();

				let start_char = node
					.get_property("start_char")
					.and_then(|v| match v {
						Value::U64(u) => Some(*u),
						_ => None,
					})
					.unwrap_or(0);

				let end_char = node
					.get_property("end_char")
					.and_then(|v| match v {
						Value::U64(u) => Some(*u),
						_ => None,
					})
					.unwrap_or(0);

				hits.push(KnowledgeHit {
					uri,
					preview,
					start_char,
					end_char,
					score,
				});
			}
		}

		Ok(hits)
	}
}
