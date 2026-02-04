//! Editor-level undo/redo with view state restoration.
//!
//! Document history is managed at the document level (text content only).
//! Editor-level history captures view state (cursor, selection, scroll)
//! so that undo/redo restores the exact editing context.
//!
//! # Architecture
//!
//! The undo system has two layers:
//!
//! - **Document layer**: Each document has its own undo stack storing text content.
//! - **Editor layer**: The [`UndoManager`] stores view state (cursor, selection, scroll)
//!   for all buffers affected by an edit.
//!
//! For broker-shared documents, undo/redo delegates to the broker and applies
//! the resulting deltas through shared-state events; local history is used for
//! view state restoration and grouping only.
//!
//! [`UndoManager`]: crate::types::UndoManager

use xeno_broker_proto::types::SharedApplyKind;

use super::undo_host::EditorUndoHost;
use crate::buffer::Buffer;
use crate::impls::{Editor, ViewSnapshot};

impl Buffer {
	/// Creates a snapshot of this buffer's view state.
	pub fn snapshot_view(&self) -> ViewSnapshot {
		ViewSnapshot {
			cursor: self.cursor,
			selection: self.selection.clone(),
			scroll_line: self.scroll_line,
			scroll_segment: self.scroll_segment,
		}
	}

	/// Restores view state from a snapshot.
	pub fn restore_view(&mut self, snapshot: &ViewSnapshot) {
		self.cursor = snapshot.cursor;
		self.selection = snapshot.selection.clone();
		self.scroll_line = snapshot.scroll_line;
		self.scroll_segment = snapshot.scroll_segment;
		self.ensure_valid_selection();
	}
}

impl Editor {
	/// Undoes the last change, restoring view state for all affected buffers.
	pub fn undo(&mut self) {
		let focused_view = self.focused_view();
		let doc_id = self
			.state
			.core
			.buffers
			.get_buffer(focused_view)
			.map(|b| b.document_id());

		#[cfg(feature = "lsp")]
		if let Some(doc_id) = doc_id
			&& let Some(uri) = self.state.shared_state.uri_for_doc_id(doc_id)
		{
			let uri_s = uri.to_string();
			// Unconditionally try broker path for shared docs to support blind undo.
			let started = {
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					lsp: &mut self.state.lsp,
					shared_state: &mut self.state.shared_state,
				};

				if core.undo_manager.can_undo() {
					core.undo_manager.start_remote_undo(&mut host)
				} else {
					if core.undo_manager.start_blind_remote_history(
						&mut host,
						crate::types::HistoryKind::Undo,
						vec![doc_id],
					) {
						Some(vec![doc_id])
					} else {
						None
					}
				}
			};

			if let Some(doc_ids) = started {
				let mut items = Vec::new();
				for id in &doc_ids {
					if let Some(u) = self.state.shared_state.uri_for_doc_id(*id) {
						items.push(u.to_string());
					}
				}

				let needs_resync = items.iter().any(|u| {
					matches!(
						self.state.shared_state.ui_status_for_uri(u).1,
						crate::shared_state::SyncStatus::NeedsResync
					)
				});
				if needs_resync {
					let core = &mut self.state.core;
					core.undo_manager.cancel_pending_history_silent();
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let needs_owner = items.iter().any(|u| !self.state.shared_state.is_owner(u));
				if needs_owner {
					for u in &items {
						self.state
							.shared_state
							.queue_history(u, SharedApplyKind::Undo);
						self.maybe_request_shared_focus(u);
					}
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let in_flight = items
					.iter()
					.any(|u| self.state.shared_state.is_in_flight(u));
				if in_flight {
					for u in items {
						self.state
							.shared_state
							.queue_history(&u, SharedApplyKind::Undo);
					}
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let mut payloads = Vec::new();
				let mut ok = true;
				for u in items {
					if let Some(payload) = self.state.shared_state.prepare_undo(&u) {
						payloads.push(payload);
					} else {
						ok = false;
						break;
					}
				}

				if ok && !payloads.is_empty() {
					for payload in payloads {
						let _ = self.state.lsp.shared_state_out_tx().send(payload);
					}
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let core = &mut self.state.core;
				core.undo_manager.cancel_pending_history_silent();
				return;
			}
		}

		let core = &mut self.state.core;
		let mut host = EditorUndoHost {
			buffers: &mut core.buffers,
			focused_view,
			config: &self.state.config,
			frame: &mut self.state.frame,
			notifications: &mut self.state.notifications,
			syntax_manager: &mut self.state.syntax_manager,
			#[cfg(feature = "lsp")]
			lsp: &mut self.state.lsp,
			#[cfg(feature = "lsp")]
			shared_state: &mut self.state.shared_state,
		};
		core.undo_manager.undo(&mut host);
	}

	/// Redoes the last undone change, restoring view state for all affected buffers.
	pub fn redo(&mut self) {
		let focused_view = self.focused_view();
		let doc_id = self
			.state
			.core
			.buffers
			.get_buffer(focused_view)
			.map(|b| b.document_id());

		#[cfg(feature = "lsp")]
		if let Some(doc_id) = doc_id
			&& let Some(uri) = self.state.shared_state.uri_for_doc_id(doc_id)
		{
			let uri_s = uri.to_string();
			let started = {
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					lsp: &mut self.state.lsp,
					shared_state: &mut self.state.shared_state,
				};

				if core.undo_manager.can_redo() {
					core.undo_manager.start_remote_redo(&mut host)
				} else {
					if core.undo_manager.start_blind_remote_history(
						&mut host,
						crate::types::HistoryKind::Redo,
						vec![doc_id],
					) {
						Some(vec![doc_id])
					} else {
						None
					}
				}
			};

			if let Some(doc_ids) = started {
				let mut items = Vec::new();
				for id in &doc_ids {
					if let Some(u) = self.state.shared_state.uri_for_doc_id(*id) {
						items.push(u.to_string());
					}
				}

				let needs_resync = items.iter().any(|u| {
					matches!(
						self.state.shared_state.ui_status_for_uri(u).1,
						crate::shared_state::SyncStatus::NeedsResync
					)
				});
				if needs_resync {
					let core = &mut self.state.core;
					core.undo_manager.cancel_pending_history_silent();
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let needs_owner = items.iter().any(|u| !self.state.shared_state.is_owner(u));
				if needs_owner {
					for u in &items {
						self.state
							.shared_state
							.queue_history(u, SharedApplyKind::Redo);
						self.maybe_request_shared_focus(u);
					}
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let in_flight = items
					.iter()
					.any(|u| self.state.shared_state.is_in_flight(u));
				if in_flight {
					for u in items {
						self.state
							.shared_state
							.queue_history(&u, SharedApplyKind::Redo);
					}
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let mut payloads = Vec::new();
				let mut ok = true;
				for u in items {
					if let Some(payload) = self.state.shared_state.prepare_redo(&u) {
						payloads.push(payload);
					} else {
						ok = false;
						break;
					}
				}

				if ok && !payloads.is_empty() {
					for payload in payloads {
						let _ = self.state.lsp.shared_state_out_tx().send(payload);
					}
					self.update_readonly_for_shared_state(&uri_s);
					return;
				}

				let core = &mut self.state.core;
				core.undo_manager.cancel_pending_history_silent();
				return;
			}
		}

		let core = &mut self.state.core;
		let mut host = EditorUndoHost {
			buffers: &mut core.buffers,
			focused_view,
			config: &self.state.config,
			frame: &mut self.state.frame,
			notifications: &mut self.state.notifications,
			syntax_manager: &mut self.state.syntax_manager,
			#[cfg(feature = "lsp")]
			lsp: &mut self.state.lsp,
			#[cfg(feature = "lsp")]
			shared_state: &mut self.state.shared_state,
		};
		core.undo_manager.redo(&mut host);
	}
}

#[cfg(test)]
mod tests;
