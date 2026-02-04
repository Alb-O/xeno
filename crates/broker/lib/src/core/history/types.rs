use helix_db::helix_engine::types::EngineError;
use ropey::Rope;
use xeno_broker_proto::types::{SyncEpoch, SyncSeq};

/// Metadata stored for a document's history graph.
#[derive(Debug, Clone)]
pub struct HistoryMeta {
	/// Current head node id.
	pub head_id: u64,
	/// Root node id for the document.
	pub root_id: u64,
	/// Next available node id.
	pub next_id: u64,
	/// Total number of history nodes tracked.
	pub history_nodes: u64,
	/// Group identifier of the current history head.
	pub head_group_id: u64,
}

/// Stored document state and history metadata.
#[derive(Debug, Clone)]
pub struct StoredDoc {
	/// Persisted history metadata.
	pub meta: HistoryMeta,
	/// Current epoch for ownership fencing.
	pub epoch: SyncEpoch,
	/// Current sequence number.
	pub seq: SyncSeq,
	/// Document length in chars.
	pub len_chars: u64,
	/// Hash of the document content.
	pub hash64: u64,
	/// Full document content as a rope.
	pub rope: Rope,
}

/// Errors produced by the history store.
#[derive(Debug)]
pub enum HistoryError {
	/// Helix-db storage errors.
	Heed(heed3::Error),
	/// Helix traversal engine errors.
	Engine(EngineError),
	/// Serialization errors for stored deltas.
	Serde(serde_json::Error),
	/// Delta could not be converted or applied.
	InvalidDelta,
	/// History graph is missing required nodes or contains cycles.
	Corrupt(String),
}

impl HistoryError {
	/// Returns true if the underlying storage error is a duplicate-key insert (LMDB MDB_KEYEXIST).
	pub fn is_duplicate_key(&self) -> bool {
		let s = self.to_string();
		s.contains("MDB_KEYEXIST") || s.contains("KEYEXIST") || s.contains("Duplicate key")
	}
}

impl std::fmt::Display for HistoryError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Heed(err) => write!(f, "{err}"),
			Self::Engine(err) => write!(f, "{err}"),
			Self::Serde(err) => write!(f, "{err}"),
			Self::InvalidDelta => write!(f, "invalid history delta"),
			Self::Corrupt(msg) => write!(f, "{msg}"),
		}
	}
}

impl std::error::Error for HistoryError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Heed(err) => Some(err),
			Self::Engine(err) => Some(err),
			Self::Serde(err) => Some(err),
			_ => None,
		}
	}
}

impl From<EngineError> for HistoryError {
	fn from(err: EngineError) -> Self {
		Self::Engine(err)
	}
}

impl From<heed3::Error> for HistoryError {
	fn from(err: heed3::Error) -> Self {
		Self::Heed(err)
	}
}

impl From<serde_json::Error> for HistoryError {
	fn from(err: serde_json::Error) -> Self {
		Self::Serde(err)
	}
}
