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
//! - Document layer: Each document has its own undo stack storing text content.
//! - Editor layer: The [`UndoManager`] stores view state (cursor, selection, scroll)
//!   for all buffers affected by an edit.
//!
//! [`UndoManager`]: crate::types::UndoManager

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
		};
		core.undo_manager.undo(&mut host);
	}

	/// Redoes the last undone change, restoring view state for all affected buffers.
	pub fn redo(&mut self) {
		let focused_view = self.focused_view();

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
		};
		core.undo_manager.redo(&mut host);
	}
}

#[cfg(test)]
mod tests;
