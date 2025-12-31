//! Text editing operations.
//!
//! Insert, delete, yank, paste, and transaction application.

use evildoer_base::Transaction;

use super::Editor;
use crate::buffer::BufferView;

impl Editor {
	pub fn insert_text(&mut self, text: &str) {
		if self.buffer().mode() == evildoer_manifest::Mode::Insert {
			self.save_insert_undo_state();
		} else {
			self.save_undo_state();
		}
		let tx = self.buffer_mut().insert_text(text);
		self.sync_sibling_selections(&tx);
		if let BufferView::Text(id) = self.buffers.focused_view() {
			self.dirty_buffers.insert(id);
		}
	}

	pub fn yank_selection(&mut self) {
		if let Some((text, count)) = self.buffer_mut().yank_selection() {
			self.registers.yank = text;
			self.notify("info", format!("Yanked {} chars", count));
		}
	}

	pub fn paste_after(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		self.save_undo_state();
		let yank = self.registers.yank.clone();
		if let Some(tx) = self.buffer_mut().paste_after(&yank) {
			self.sync_sibling_selections(&tx);
		}
		if let BufferView::Text(id) = self.buffers.focused_view() {
			self.dirty_buffers.insert(id);
		}
	}

	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		self.save_undo_state();
		let yank = self.registers.yank.clone();
		if let Some(tx) = self.buffer_mut().paste_before(&yank) {
			self.sync_sibling_selections(&tx);
		}
		if let BufferView::Text(id) = self.buffers.focused_view() {
			self.dirty_buffers.insert(id);
		}
	}

	pub fn delete_selection(&mut self) {
		if self.buffer().selection.primary().is_empty() {
			return;
		}
		self.save_undo_state();
		if let Some(tx) = self.buffer_mut().delete_selection() {
			self.sync_sibling_selections(&tx);
			if let BufferView::Text(id) = self.buffers.focused_view() {
				self.dirty_buffers.insert(id);
			}
		}
	}

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
