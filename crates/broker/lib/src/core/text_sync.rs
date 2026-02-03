//! Text synchronization gating logic.
//!
//! Enforces single-writer ownership per document URI for LSP text sync notifications.

use std::collections::HashMap;

use xeno_broker_proto::types::{DocId, SessionId};

/// Cached diagnostics payload for a document.
#[derive(Debug, Clone)]
pub struct DocDiagnostics {
	/// Optional document version from the LSP server.
	pub version: Option<u32>,
	/// Diagnostics JSON payload.
	pub diagnostics: String,
}

/// Registry of document identities and versions for an LSP server.
#[derive(Debug, Default)]
pub struct DocRegistry {
	/// Map of URI to (DocId, last_version).
	pub by_uri: HashMap<String, (DocId, u32)>,
	/// Cached diagnostics keyed by URI.
	pub diagnostics_by_uri: HashMap<String, DocDiagnostics>,
	next_doc_id: u64,
}

impl DocRegistry {
	/// Updates or registers a document version.
	pub fn update(&mut self, uri: String, version: u32) {
		if let Some(info) = self.by_uri.get_mut(&uri) {
			info.1 = version;
		} else {
			let id = DocId(self.next_doc_id);
			self.next_doc_id += 1;
			self.by_uri.insert(uri, (id, version));
		}
	}

	/// Removes document identity state without clearing cached diagnostics.
	pub fn remove(&mut self, uri: &str) {
		self.by_uri.remove(uri);
	}

	/// Clears cached diagnostics for a document.
	pub fn clear_diagnostics(&mut self, uri: &str) {
		self.diagnostics_by_uri.remove(uri);
	}

	/// Stores the last diagnostics payload for a document.
	pub fn update_diagnostics(&mut self, uri: String, version: Option<u32>, diagnostics: String) {
		self.diagnostics_by_uri.insert(
			uri,
			DocDiagnostics {
				version,
				diagnostics,
			},
		);
	}

	/// Returns the cached diagnostics payload for a document.
	pub fn cached_diagnostics(&self, uri: &str) -> Option<&DocDiagnostics> {
		self.diagnostics_by_uri.get(uri)
	}
}

/// Registry of writer ownership for open documents.
#[derive(Debug, Default)]
pub struct DocOwnerRegistry {
	/// Map of URI to writer state.
	pub by_uri: HashMap<String, DocOwnerState>,
}

/// Writer state for an open document.
#[derive(Debug)]
pub struct DocOwnerState {
	/// Session that currently owns the document.
	pub owner: SessionId,
	/// Per-session open reference counts.
	pub open_refcounts: HashMap<SessionId, u32>,
	/// Last observed version from the owner.
	pub last_version: u32,
}

/// Result of gating a text sync notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocGateDecision {
	/// Forward to server: session is owner or first open.
	Forward,
	/// Drop silently: session is follower.
	DropSilently,
	/// Reject: session is not permitted to sync.
	RejectNotOwner,
}

/// Result struct for document gating operations.
pub struct DocGateResult {
	/// Decision (allow/reject).
	pub decision: DocGateDecision,
	/// URI being gated.
	pub uri: String,
	/// Kind of operation.
	pub kind: DocGateKind,
}

/// Kind of text sync operation.
pub enum DocGateKind {
	/// Document opened.
	DidOpen {
		/// New version.
		version: u32,
	},
	/// Document changed.
	DidChange {
		/// New version.
		version: u32,
	},
	/// Document closed.
	DidClose,
}
