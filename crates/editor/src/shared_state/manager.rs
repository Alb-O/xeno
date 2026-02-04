//! Core [`SharedStateManager`] struct and query methods.

use std::collections::HashMap;

use xeno_broker_proto::types::DocSyncPhase;

use super::types::{SharedDocEntry, SharedStateRole, SyncStatus};
use crate::buffer::DocumentId;

/// Manages broker-backed shared state for all open documents.
pub struct SharedStateManager {
	pub(super) docs: HashMap<String, SharedDocEntry>,
	pub(super) uri_to_doc_id: HashMap<String, DocumentId>,
	pub(super) doc_id_to_uri: HashMap<DocumentId, String>,
}

impl SharedStateManager {
	/// Creates a new empty manager.
	pub fn new() -> Self {
		Self {
			docs: HashMap::new(),
			uri_to_doc_id: HashMap::new(),
			doc_id_to_uri: HashMap::new(),
		}
	}

	/// Returns true if mutations for the URI are currently prohibited.
	pub fn is_edit_blocked(&self, uri: &str) -> bool {
		self.docs.get(uri).is_some_and(SharedDocEntry::is_blocked)
	}

	/// Returns true if the local session is the current owner.
	pub fn is_owner(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|entry| entry.role == SharedStateRole::Owner)
	}

	/// Returns true if the document is currently unlocked.
	pub fn is_unlocked(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|entry| entry.phase == DocSyncPhase::Unlocked)
	}

	/// Returns the local role for a document URI.
	pub fn role_for_uri(&self, uri: &str) -> Option<SharedStateRole> {
		self.docs.get(uri).map(|entry| entry.role)
	}

	/// Returns the current sync status for UI display.
	pub fn ui_status_for_uri(&self, uri: &str) -> (Option<SharedStateRole>, SyncStatus) {
		let Some(entry) = self.docs.get(uri) else {
			return (None, SyncStatus::Off);
		};

		let status = if entry.needs_resync {
			SyncStatus::NeedsResync
		} else if entry.phase == DocSyncPhase::Unlocked {
			SyncStatus::Unlocked
		} else if entry.role == SharedStateRole::Owner {
			SyncStatus::Owner
		} else {
			SyncStatus::Follower
		};
		(Some(entry.role), status)
	}

	/// Returns the URI for a document id.
	pub fn uri_for_doc_id(&self, doc_id: DocumentId) -> Option<&str> {
		self.doc_id_to_uri.get(&doc_id).map(String::as_str)
	}

	/// Returns the doc id for a document URI.
	pub fn doc_id_for_uri(&self, uri: &str) -> Option<DocumentId> {
		self.uri_to_doc_id.get(uri).copied()
	}

	/// Disables all shared state tracking and clears document mappings.
	pub fn disable_all(&mut self) {
		self.docs.clear();
		self.uri_to_doc_id.clear();
		self.doc_id_to_uri.clear();
	}

	/// Caches view snapshots for a local edit group.
	pub fn cache_view_group(
		&mut self,
		uri: &str,
		group_id: u64,
		pre: HashMap<crate::buffer::ViewId, crate::types::ViewSnapshot>,
		post: HashMap<crate::buffer::ViewId, crate::types::ViewSnapshot>,
	) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry
				.view_history
				.groups
				.insert(group_id, super::types::GroupViewState { pre, post });
		}
	}

	/// Retrieves cached view state for a group.
	pub fn get_view_group(
		&self,
		uri: &str,
		group_id: u64,
	) -> Option<&super::types::GroupViewState> {
		self.docs.get(uri)?.view_history.groups.get(&group_id)
	}

	/// Returns the current local undo group ID.
	pub fn current_undo_group(&self, uri: &str) -> u64 {
		self.docs
			.get(uri)
			.map(|e| e.current_undo_group)
			.unwrap_or(0)
	}

	/// Returns true if a shared document has an in-flight mutation.
	pub fn is_in_flight(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|entry| entry.in_flight.is_some())
	}

	/// Queues a history operation for a shared document.
	pub fn queue_history(&mut self, uri: &str, kind: xeno_broker_proto::types::SharedApplyKind) {
		if let Some(entry) = self.docs.get_mut(uri) {
			entry.pending_history.push_back(kind);
		}
	}

	#[cfg(test)]
	pub(crate) fn pending_history_len(&self, uri: &str) -> usize {
		self.docs
			.get(uri)
			.map(|entry| entry.pending_history.len())
			.unwrap_or(0)
	}

	#[cfg(test)]
	pub(crate) fn has_pending_align(&self, uri: &str) -> bool {
		self.docs
			.get(uri)
			.is_some_and(|entry| entry.pending_align.is_some())
	}
}

impl Default for SharedStateManager {
	fn default() -> Self {
		Self::new()
	}
}
