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
//! The [`UndoHost`] trait abstracts the Editor operations needed by UndoManager,
//! enabling cleaner separation of concerns.
//!
//! [`UndoManager`]: crate::types::UndoManager
//! [`UndoHost`]: crate::types::UndoHost

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
		#[cfg(feature = "lsp")]
		let broker_uris = self
			.state
			.core
			.undo_manager
			.last_undo_group()
			.and_then(|group| {
				let mut uris = Vec::new();
				for doc_id in &group.affected_docs {
					let uri = self.state.shared_state.uri_for_doc_id(*doc_id)?;
					if !self.state.shared_state.can_prepare_history(uri) {
						return None;
					}
					uris.push(uri.to_string());
				}
				Some(uris)
			});

		let focused_view = self.focused_view();
		#[cfg(feature = "lsp")]
		if let Some(uris) = broker_uris {
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
				core.undo_manager.start_remote_undo(&mut host)
			};

			if started.is_some() {
				let mut ok = true;
				for uri in &uris {
					if let Some(payload) = self.state.shared_state.prepare_undo(uri) {
						let _ = self.state.lsp.shared_state_out_tx().send(payload);
					} else {
						ok = false;
						break;
					}
				}

				if ok {
					for uri in &uris {
						self.update_readonly_for_shared_state(uri);
					}
					return;
				}

				for uri in &uris {
					self.state.shared_state.cancel_history_in_flight(uri);
				}

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
				core.undo_manager
					.cancel_pending_history(&mut host, crate::types::HistoryKind::Undo);
				return;
			}
			return;
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
		#[cfg(feature = "lsp")]
		let broker_uris = self
			.state
			.core
			.undo_manager
			.last_redo_group()
			.and_then(|group| {
				let mut uris = Vec::new();
				for doc_id in &group.affected_docs {
					let uri = self.state.shared_state.uri_for_doc_id(*doc_id)?;
					if !self.state.shared_state.can_prepare_history(uri) {
						return None;
					}
					uris.push(uri.to_string());
				}
				Some(uris)
			});

		let focused_view = self.focused_view();
		#[cfg(feature = "lsp")]
		if let Some(uris) = broker_uris {
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
				core.undo_manager.start_remote_redo(&mut host)
			};

			if started.is_some() {
				let mut ok = true;
				for uri in &uris {
					if let Some(payload) = self.state.shared_state.prepare_redo(uri) {
						let _ = self.state.lsp.shared_state_out_tx().send(payload);
					} else {
						ok = false;
						break;
					}
				}

				if ok {
					for uri in &uris {
						self.update_readonly_for_shared_state(uri);
					}
					return;
				}

				for uri in &uris {
					self.state.shared_state.cancel_history_in_flight(uri);
				}

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
				core.undo_manager
					.cancel_pending_history(&mut host, crate::types::HistoryKind::Redo);
				return;
			}
			return;
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
