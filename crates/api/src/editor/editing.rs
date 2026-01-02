//! Text editing operations.
//!
//! Insert, delete, yank, paste, and transaction application.

use evildoer_base::Transaction;

use super::Editor;
use crate::buffer::BufferView;

impl Editor {
	/// Inserts text at the current cursor position(s).
	pub fn insert_text(&mut self, text: &str) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

		if self.buffer().mode() == evildoer_base::Mode::Insert {
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

		{
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			buffer.finalize_selection(new_selection);
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Copies the current selection to the yank register.
	pub fn yank_selection(&mut self) {
		if let Some((text, count)) = self.buffer_mut().yank_selection() {
			self.registers.yank = text;
			self.notify("info", format!("Yanked {} chars", count));
		}
	}

	/// Pastes the yank register content after the cursor.
	pub fn paste_after(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}

		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

		self.save_undo_state();
		let yank = self.registers.yank.clone();

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

		{
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			buffer.finalize_selection(new_selection);
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Pastes the yank register content before the cursor.
	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}

		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

		self.save_undo_state();
		let yank = self.registers.yank.clone();

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

		{
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			buffer.finalize_selection(new_selection);
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Deletes the currently selected text.
	pub fn delete_selection(&mut self) {
		if self.buffer().selection.primary().is_empty() {
			return;
		}

		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

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

		{
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			buffer.finalize_selection(new_selection);
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Applies a transaction to the focused buffer.
	pub fn apply_transaction(&mut self, tx: &Transaction) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};
		self.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
			.apply_transaction_with_syntax(tx, &self.language_loader);
		self.dirty_buffers.insert(buffer_id);
		self.sync_sibling_selections(tx);
	}

	/// Triggers a full syntax reparse of the focused buffer.
	pub fn reparse_syntax(&mut self) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.language_loader);
	}
}
