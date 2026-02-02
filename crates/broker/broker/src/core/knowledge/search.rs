use bumpalo::Bump;
use helix_db::helix_engine::traversal_core::ops::bm25::search_bm25::SearchBM25Adapter;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::helix_engine::types::{EngineError, StorageError};
use helix_db::protocol::value::Value;
use xeno_broker_proto::types::KnowledgeHit;

use super::{KnowledgeCore, KnowledgeError};

const LABEL_CHUNK: &str = "Chunk";
const PREVIEW_CHARS: usize = 200;

impl KnowledgeCore {
	pub fn search(&self, query: &str, limit: u32) -> Result<Vec<KnowledgeHit>, KnowledgeError> {
		if query.trim().is_empty() || limit == 0 {
			return Ok(Vec::new());
		}

		let arena = Bump::new();
		let txn = self
			.storage()
			.graph_env
			.read_txn()
			.map_err(helix_db::helix_engine::types::EngineError::from)?;

		let results =
			match G::new(self.storage(), &txn, &arena).search_bm25(LABEL_CHUNK, query, limit) {
				Ok(results) => results,
				Err(err) => {
					if is_empty_bm25(&err) {
						return Ok(Vec::new());
					}
					return Err(err.into());
				}
			};

		let mut hits = Vec::new();

		for entry in results {
			let tv = entry?;
			let TraversalValue::NodeWithScore { node, score } = tv else {
				continue;
			};

			let uri = match node.get_property("doc_uri") {
				Some(Value::String(value)) => value.clone(),
				_ => continue,
			};
			let start_char = match node.get_property("start_char") {
				Some(Value::U64(value)) => *value,
				_ => continue,
			};
			let end_char = match node.get_property("end_char") {
				Some(Value::U64(value)) => *value,
				_ => continue,
			};
			let text = match node.get_property("text") {
				Some(Value::String(value)) => value.as_str(),
				_ => "",
			};

			hits.push(KnowledgeHit {
				uri,
				start_char,
				end_char,
				score,
				preview: make_preview(text, PREVIEW_CHARS),
			});
		}

		Ok(hits)
	}
}

fn make_preview(text: &str, max_chars: usize) -> String {
	let mut preview = String::new();
	let mut count = 0usize;

	for ch in text.chars() {
		if count >= max_chars {
			break;
		}
		let ch = match ch {
			'\n' | '\r' | '\t' => ' ',
			_ => ch,
		};
		preview.push(ch);
		count += 1;
	}

	preview
}

fn is_empty_bm25(err: &EngineError) -> bool {
	matches!(
		err,
		EngineError::Storage(StorageError::Backend(msg))
			if msg.contains("BM25 metadata not found")
	)
}
