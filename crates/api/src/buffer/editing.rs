//! Text editing operations for buffers.

use tome_base::Transaction;
use tome_language::LanguageLoader;
use tome_manifest::Mode;
use tome_stdlib::movement;

use super::Buffer;

impl Buffer {
	/// Inserts text at all cursor positions.
	///
	/// In insert mode, this groups consecutive inserts into a single undo.
	pub fn insert_text(&mut self, text: &str) {
		if self.mode() == Mode::Insert {
			self.save_insert_undo_state();
		} else {
			self.save_undo_state();
		}

		// Collapse all selections to their insertion points
		let mut insertion_points = self.selection.clone();
		insertion_points.transform_mut(|r| {
			let pos = r.min();
			r.anchor = pos;
			r.head = pos;
		});

		let tx = Transaction::insert(self.doc.slice(..), &insertion_points, text.to_string());
		let mut new_selection = tx.map_selection(&insertion_points);
		new_selection.transform_mut(|r| {
			let pos = r.max();
			r.anchor = pos;
			r.head = pos;
		});
		self.apply_transaction(&tx);

		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
	}

	/// Yanks (copies) the primary selection to the provided register string.
	///
	/// Returns the yanked text and count, or None if selection is empty.
	pub fn yank_selection(&self) -> Option<(String, usize)> {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			let text = self.doc.slice(from..to).to_string();
			let count = to - from;
			Some((text, count))
		} else {
			None
		}
	}

	/// Pastes text after the cursor position.
	pub fn paste_after(&mut self, text: &str) {
		if text.is_empty() {
			return;
		}
		let slice = self.doc.slice(..);
		self.selection.transform_mut(|r| {
			*r = movement::move_horizontally(
				slice,
				*r,
				tome_base::range::Direction::Forward,
				1,
				false,
			);
		});
		self.insert_text(text);
	}

	/// Pastes text before the cursor position.
	pub fn paste_before(&mut self, text: &str) {
		if text.is_empty() {
			return;
		}
		self.insert_text(text);
	}

	/// Deletes the current selection.
	///
	/// Returns true if anything was deleted.
	pub fn delete_selection(&mut self) -> bool {
		if !self.selection.primary().is_empty() {
			self.save_undo_state();
			let tx = Transaction::delete(self.doc.slice(..), &self.selection);
			self.selection = tx.map_selection(&self.selection);
			self.apply_transaction(&tx);
			true
		} else {
			false
		}
	}

	/// Applies a transaction to the document with incremental syntax tree update.
	///
	/// This is the central method for all document modifications.
	pub fn apply_transaction(&mut self, tx: &Transaction) {
		tx.apply(&mut self.doc);
		self.modified = true;
	}

	/// Applies a transaction and updates syntax tree incrementally.
	pub fn apply_transaction_with_syntax(
		&mut self,
		tx: &Transaction,
		language_loader: &LanguageLoader,
	) {
		let old_doc = self.doc.clone();
		tx.apply(&mut self.doc);

		if let Some(ref mut syntax) = self.syntax {
			let _ = syntax.update_from_changeset(
				old_doc.slice(..),
				self.doc.slice(..),
				tx.changes(),
				language_loader,
			);
		}

		self.modified = true;
	}
}
