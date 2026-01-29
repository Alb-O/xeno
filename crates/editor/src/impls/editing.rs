//! Text editing operations.
//!
//! Insert, delete, yank, paste, and transaction application.

use xeno_primitives::{EditOrigin, Selection, Transaction, UndoPolicy};
use xeno_registry::notifications::keys;

use super::Editor;
use super::undo_host::EditorUndoHost;
use crate::buffer::ViewId;

impl Editor {
	pub(crate) fn guard_readonly(&mut self) -> bool {
		if self.buffer().is_readonly() {
			self.notify(keys::BUFFER_READONLY);
			return false;
		}
		true
	}

	/// Applies a transaction with full undo support.
	///
	/// This is the primary edit method that:
	/// 1. Prepares the edit via `UndoManager` (captures view snapshots)
	/// 2. Applies the transaction with the specified undo policy
	/// 3. Finalizes via `UndoManager` (pushes `EditorUndoGroup` if needed)
	pub(crate) fn apply_edit(
		&mut self,
		buffer_id: ViewId,
		tx: &Transaction,
		new_selection: Option<Selection>,
		undo: UndoPolicy,
		origin: EditOrigin,
	) -> bool {
		let focused_view = self.focused_view();
		let core = &mut self.state.core;
		let undo_manager = &mut core.undo_manager;
		let mut host = EditorUndoHost {
			buffers: &mut core.buffers,
			focused_view,
			config: &self.state.config,
			frame: &mut self.state.frame,
			notifications: &mut self.state.notifications,
			#[cfg(feature = "lsp")]
			lsp: &mut self.state.lsp,
		};

		let res = undo_manager.with_edit(&mut host, buffer_id, undo, origin, |host| {
			host.apply_transaction_inner(buffer_id, tx, new_selection, undo)
		});

		if res {
			let mut layers = std::mem::take(&mut self.state.overlay_system.layers);
			layers.notify_event(self, crate::overlay::LayerEvent::BufferEdited(buffer_id));
			self.state.overlay_system.layers = layers;
		}
		res
	}

	/// Inserts a newline with smart indentation.
	///
	/// Copies the leading whitespace from the current line to the new line.
	pub fn insert_newline_with_indent(&mut self) {
		let indent = {
			let buffer = self.buffer();
			let cursor = buffer.cursor;
			buffer.with_doc(|doc| {
				let line_idx = doc.content().char_to_line(cursor);
				let line = doc.content().line(line_idx);

				line.chars()
					.take_while(|c| *c == ' ' || *c == '\t')
					.collect::<String>()
			})
		};

		let text = format!("\n{}", indent);
		self.insert_text(&text);
	}

	/// Inserts text at the current cursor position(s).
	pub fn insert_text(&mut self, text: &str) {
		let buffer_id = self.focused_view();

		if !self.guard_readonly() {
			return;
		}

		let undo = if self.buffer().mode() == xeno_primitives::Mode::Insert {
			UndoPolicy::MergeWithCurrentGroup
		} else {
			UndoPolicy::Record
		};

		let (tx, new_selection) = {
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_insert(text)
		};

		let applied = self.apply_edit(
			buffer_id,
			&tx,
			Some(new_selection),
			undo,
			EditOrigin::Internal("insert"),
		);

		if !applied {
			self.notify(keys::BUFFER_READONLY);
		}
	}

	/// Copies the current selection to the yank register.
	pub fn yank_selection(&mut self) {
		if let Some(yank) = self.buffer_mut().yank_selection() {
			let count = yank.total_chars;
			self.state.core.workspace.registers.yank = yank;
			self.notify(keys::yanked_chars(count));
		}
	}

	/// Pastes the yank register content after the cursor.
	pub fn paste_after(&mut self) {
		if self.state.core.workspace.registers.yank.is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.focused_view();
		let yank = self.state.core.workspace.registers.yank.joined();

		let Some((tx, new_selection)) = ({
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_paste_after(&yank)
		}) else {
			return;
		};

		let applied = self.apply_edit(
			buffer_id,
			&tx,
			Some(new_selection),
			UndoPolicy::Record,
			EditOrigin::Internal("paste"),
		);

		if !applied {
			self.notify(keys::BUFFER_READONLY);
		}
	}

	/// Pastes the yank register content before the cursor.
	pub fn paste_before(&mut self) {
		if self.state.core.workspace.registers.yank.is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.focused_view();
		let yank = self.state.core.workspace.registers.yank.joined();

		let Some((tx, new_selection)) = ({
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_paste_before(&yank)
		}) else {
			return;
		};

		let applied = self.apply_edit(
			buffer_id,
			&tx,
			Some(new_selection),
			UndoPolicy::Record,
			EditOrigin::Internal("paste"),
		);

		if !applied {
			self.notify(keys::BUFFER_READONLY);
		}
	}

	/// Deletes the currently selected text.
	pub fn delete_selection(&mut self) {
		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.focused_view();

		let Some((tx, new_selection)) = ({
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_delete_selection()
		}) else {
			return;
		};

		let applied = self.apply_edit(
			buffer_id,
			&tx,
			Some(new_selection),
			UndoPolicy::Record,
			EditOrigin::Internal("delete"),
		);

		if !applied {
			self.notify(keys::BUFFER_READONLY);
		}
	}

	/// Triggers a full syntax reparse of the focused buffer.
	///
	/// Accesses the buffer directly rather than through `self.buffer_mut()` to
	/// avoid a borrow conflict with `self.state.config.language_loader`.
	pub fn reparse_syntax(&mut self) {
		let buffer_id = self.focused_view();
		let buffer = self
			.state
			.core
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.state.config.language_loader);
	}
}
