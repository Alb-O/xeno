//! Text editing operations.
//!
//! Insert, delete, yank, paste, and transaction application.

use xeno_base::Transaction;
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

	/// Inserts text at the current cursor position(s).
	pub fn insert_text(&mut self, text: &str) {
		let buffer_id = self.buffers.focused_view();

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

		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			if applied {
				buffer.finalize_selection(new_selection);
			}
			applied
		};

		if !applied {
			self.notify(keys::buffer_readonly);
			return;
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Copies the current selection to the yank register.
	pub fn yank_selection(&mut self) {
		if let Some((text, count)) = self.buffer_mut().yank_selection() {
			self.registers.yank = text;
			self.notify(keys::yanked_chars::call(count));
		}
	}

	/// Pastes the yank register content after the cursor.
	pub fn paste_after(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.buffers.focused_view();

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

		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			if applied {
				buffer.finalize_selection(new_selection);
			}
			applied
		};

		if !applied {
			self.notify(keys::buffer_readonly);
			return;
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Pastes the yank register content before the cursor.
	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.buffers.focused_view();

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

		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			if applied {
				buffer.finalize_selection(new_selection);
			}
			applied
		};

		if !applied {
			self.notify(keys::buffer_readonly);
			return;
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Deletes the currently selected text.
	pub fn delete_selection(&mut self) {
		if self.buffer().selection.primary().is_empty() {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		let buffer_id = self.buffers.focused_view();

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

		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply_transaction_with_syntax(&tx, &self.language_loader);
			if applied {
				buffer.finalize_selection(new_selection);
			}
			applied
		};

		if !applied {
			self.notify(keys::buffer_readonly);
			return;
		}

		self.sync_sibling_selections(&tx);
		self.dirty_buffers.insert(buffer_id);
	}

	/// Applies a transaction to the focused buffer.
	pub fn apply_transaction(&mut self, tx: &Transaction) {
		let buffer_id = self.buffers.focused_view();
		let applied = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
			.apply_transaction_with_syntax(tx, &self.language_loader);
		if !applied {
			self.notify(keys::buffer_readonly);
			return;
		}
		self.dirty_buffers.insert(buffer_id);
		self.sync_sibling_selections(tx);
	}

	/// Triggers a full syntax reparse of the focused buffer.
	pub fn reparse_syntax(&mut self) {
		let buffer_id = self.buffers.focused_view();

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.language_loader);
	}
}
