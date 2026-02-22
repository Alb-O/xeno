//! Pull diagnostic state tracking.
//!
//! Tracks per-buffer in-flight requests, doc versions, and `result_id` values
//! for the `textDocument/diagnostic` pull model. Ensures correct `previous_result_id`
//! propagation and retry on failure.

use std::collections::{HashMap, HashSet};

use crate::buffer::ViewId;

/// Per-buffer pull diagnostic entry.
struct Entry {
	/// Doc version when diagnostics were last successfully received.
	doc_rev: u64,
	/// Result ID from the server's last Full or Unchanged response.
	result_id: Option<String>,
}

/// Manages pull diagnostic state for all buffers.
pub(crate) struct PullDiagState {
	entries: HashMap<ViewId, Entry>,
	in_flight: HashSet<ViewId>,
}

impl PullDiagState {
	pub fn new() -> Self {
		Self {
			entries: HashMap::new(),
			in_flight: HashSet::new(),
		}
	}

	/// Returns true if this buffer needs a pull request (new version or never requested).
	pub fn needs_request(&self, buffer_id: ViewId, doc_rev: u64) -> bool {
		if self.in_flight.contains(&buffer_id) {
			return false;
		}
		match self.entries.get(&buffer_id) {
			Some(e) => e.doc_rev != doc_rev,
			None => true,
		}
	}

	/// Returns the `previous_result_id` to send for this buffer.
	pub fn previous_result_id(&self, buffer_id: ViewId) -> Option<String> {
		self.entries.get(&buffer_id).and_then(|e| e.result_id.clone())
	}

	/// Marks a buffer as having an in-flight request.
	pub fn mark_in_flight(&mut self, buffer_id: ViewId) {
		self.in_flight.insert(buffer_id);
	}

	/// Records a successful Full response.
	pub fn record_full(&mut self, buffer_id: ViewId, doc_rev: u64, result_id: Option<String>) {
		self.in_flight.remove(&buffer_id);
		self.entries.insert(buffer_id, Entry { doc_rev, result_id });
	}

	/// Records a successful Unchanged response (keeps existing result_id).
	pub fn record_unchanged(&mut self, buffer_id: ViewId, doc_rev: u64, result_id: String) {
		self.in_flight.remove(&buffer_id);
		self.entries.insert(
			buffer_id,
			Entry {
				doc_rev,
				result_id: Some(result_id),
			},
		);
	}

	/// Records a failed request, clearing in-flight to allow retry.
	pub fn record_failed(&mut self, buffer_id: ViewId) {
		self.in_flight.remove(&buffer_id);
	}

	/// Clears all state (e.g. on `workspace/diagnostic/refresh`).
	pub fn invalidate_all(&mut self) {
		self.entries.clear();
		self.in_flight.clear();
	}
}
