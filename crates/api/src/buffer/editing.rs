//! Text editing operations for buffers.

use evildoer_base::Transaction;
use evildoer_language::LanguageLoader;
use evildoer_manifest::Mode;
use evildoer_stdlib::movement;

use super::Buffer;

impl Buffer {
	/// Inserts text at all cursor positions.
	///
	/// In insert mode, this groups consecutive inserts into a single undo.
	pub fn insert_text(&mut self, text: &str) {
		self.ensure_valid_selection();

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

		let tx = {
			let doc = self.doc();
			Transaction::insert(doc.content.slice(..), &insertion_points, text.to_string())
		};
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
	/// Returns the yanked text and count, or None if selection is empty or invalid.
	pub fn yank_selection(&mut self) -> Option<(String, usize)> {
		self.ensure_valid_selection();

		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			let doc = self.doc();
			let text = doc.content.slice(from..to).to_string();
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
		self.ensure_valid_selection();

		// Compute new ranges by moving each cursor forward by 1
		let new_ranges: Vec<_> = {
			let doc = self.doc();
			self.selection
				.ranges()
				.iter()
				.map(|r| {
					movement::move_horizontally(
						doc.content.slice(..),
						*r,
						evildoer_base::range::Direction::Forward,
						1,
						false,
					)
				})
				.collect()
		};
		self.selection =
			evildoer_base::Selection::from_vec(new_ranges, self.selection.primary_index());
		self.insert_text(text);
	}

	/// Pastes text before the cursor position.
	pub fn paste_before(&mut self, text: &str) {
		if text.is_empty() {
			return;
		}
		self.ensure_valid_selection();
		self.insert_text(text);
	}

	/// Deletes the current selection.
	///
	/// Returns true if anything was deleted.
	pub fn delete_selection(&mut self) -> bool {
		self.ensure_valid_selection();

		if !self.selection.primary().is_empty() {
			self.save_undo_state();
			let tx = {
				let doc = self.doc();
				Transaction::delete(doc.content.slice(..), &self.selection)
			};
			self.selection = tx.map_selection(&self.selection);
			self.apply_transaction(&tx);
			true
		} else {
			false
		}
	}

	/// Applies a transaction to the document. Increments the version counter.
	pub fn apply_transaction(&self, tx: &Transaction) {
		let mut doc = self.doc_mut();
		tx.apply(&mut doc.content);
		doc.modified = true;
		doc.version = doc.version.wrapping_add(1);
	}

	/// Applies a transaction and updates syntax tree incrementally.
	pub fn apply_transaction_with_syntax(
		&self,
		tx: &Transaction,
		language_loader: &LanguageLoader,
	) {
		let mut doc = self.doc_mut();
		let old_doc = doc.content.clone();
		tx.apply(&mut doc.content);

		if doc.syntax.is_some() {
			// Clone the new content to avoid borrow conflict with syntax
			let new_doc = doc.content.clone();
			if let Some(ref mut syntax) = doc.syntax {
				let _ = syntax.update_from_changeset(
					old_doc.slice(..),
					new_doc.slice(..),
					tx.changes(),
					language_loader,
				);
			}
		}

		doc.modified = true;
		doc.version = doc.version.wrapping_add(1);
	}
}
