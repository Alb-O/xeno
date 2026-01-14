//! Text editing operations.
//!
//! Insert, delete, yank, paste, and transaction application.

use xeno_base::{Selection, Transaction};
use xeno_registry_notifications::keys;

use super::Editor;

impl Editor {
	pub(crate) fn guard_readonly(&mut self) -> bool {
		if self.buffer().is_readonly() {
			self.notify(keys::buffer_readonly);
			return false;
		}
		true
	}

	fn apply_transaction_with_selection(
		&mut self,
		buffer_id: crate::buffer::BufferId,
		tx: &Transaction,
		new_selection: Option<Selection>,
	) -> bool {
		#[cfg(feature = "lsp")]
		let encoding = {
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("focused buffer must exist");
			self.lsp.incremental_encoding_for_buffer(buffer)
		};

		#[cfg(feature = "lsp")]
		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = if let Some(encoding) = encoding {
				buffer.apply_edit_with_lsp(tx, &self.config.language_loader, encoding)
			} else {
				buffer.apply_transaction_with_syntax(tx, &self.config.language_loader)
			};
			if applied && let Some(selection) = new_selection {
				buffer.finalize_selection(selection);
			}
			applied
		};

		#[cfg(not(feature = "lsp"))]
		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply_transaction_with_syntax(tx, &self.config.language_loader);
			if applied && let Some(selection) = new_selection {
				buffer.finalize_selection(selection);
			}
			applied
		};

		if applied {
			self.sync_sibling_selections(tx);
			self.frame.dirty_buffers.insert(buffer_id);
		}

		applied
	}

	/// Inserts a newline with smart indentation.
	///
	/// Copies the leading whitespace from the current line to the new line.
	pub fn insert_newline_with_indent(&mut self) {
		let indent = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			let cursor = buffer.cursor;
			let line_idx = doc.content.char_to_line(cursor);
			let line = doc.content.line(line_idx);

			line.chars()
				.take_while(|c| *c == ' ' || *c == '\t')
				.collect::<String>()
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

		if self.buffer().mode() == xeno_base::Mode::Insert {
			self.save_insert_undo_state();
		} else {
			self.save_undo_state();
		}

		// Prepare the transaction and new selection (without applying)
		let (tx, new_selection) = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_insert(text)
		};

		let applied = self.apply_transaction_with_selection(buffer_id, &tx, Some(new_selection));

		if !applied {
			self.notify(keys::buffer_readonly);
		}
	}

	/// Copies the current selection to the yank register.
	pub fn yank_selection(&mut self) {
		if let Some((text, count)) = self.buffer_mut().yank_selection() {
			self.workspace.registers.yank = text;
			self.notify(keys::yanked_chars::call(count));
		}
	}

	/// Pastes the yank register content after the cursor.
	pub fn paste_after(&mut self) {
		if self.workspace.registers.yank.is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.focused_view();

		self.save_undo_state();
		let yank = self.workspace.registers.yank.clone();

		// Prepare the transaction and new selection (without applying)
		let Some((tx, new_selection)) = ({
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_paste_after(&yank)
		}) else {
			return;
		};

		let applied = self.apply_transaction_with_selection(buffer_id, &tx, Some(new_selection));

		if !applied {
			self.notify(keys::buffer_readonly);
		}
	}

	/// Pastes the yank register content before the cursor.
	pub fn paste_before(&mut self) {
		if self.workspace.registers.yank.is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.focused_view();

		self.save_undo_state();
		let yank = self.workspace.registers.yank.clone();

		// Prepare the transaction and new selection (without applying)
		let Some((tx, new_selection)) = ({
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_paste_before(&yank)
		}) else {
			return;
		};

		let applied = self.apply_transaction_with_selection(buffer_id, &tx, Some(new_selection));

		if !applied {
			self.notify(keys::buffer_readonly);
		}
	}

	/// Deletes the currently selected text.
	pub fn delete_selection(&mut self) {
		if self.buffer().selection.primary().is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.focused_view();

		self.save_undo_state();

		// Prepare the transaction and new selection (without applying)
		let Some((tx, new_selection)) = ({
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_delete_selection()
		}) else {
			return;
		};

		let applied = self.apply_transaction_with_selection(buffer_id, &tx, Some(new_selection));

		if !applied {
			self.notify(keys::buffer_readonly);
		}
	}

	/// Applies a transaction to the focused buffer.
	pub fn apply_transaction(&mut self, tx: &Transaction) {
		let buffer_id = self.focused_view();
		let applied = self.apply_transaction_with_selection(buffer_id, tx, None);
		if !applied {
			self.notify(keys::buffer_readonly);
		}
	}

	/// Triggers a full syntax reparse of the focused buffer.
	pub fn reparse_syntax(&mut self) {
		let buffer_id = self.focused_view();

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.config.language_loader);
	}
}
