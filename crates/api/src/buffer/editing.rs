//! Text editing operations for buffers.

use xeno_base::Transaction;
use xeno_core::movement;
use xeno_language::LanguageLoader;

use super::Buffer;

impl Buffer {
	/// Inserts text at all cursor positions, returning the [`Transaction`] without applying it.
	///
	/// The caller is responsible for applying the transaction (with or without syntax update).
	pub fn prepare_insert(&mut self, text: &str) -> (Transaction, xeno_base::Selection) {
		self.ensure_valid_selection();

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

		(tx, new_selection)
	}

	/// Inserts text at all cursor positions, returning the applied [`Transaction`].
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware insertion,
	/// use [`prepare_insert`] and apply with [`apply_transaction_with_syntax`].
	pub fn insert_text(&mut self, text: &str) -> Transaction {
		let (tx, new_selection) = self.prepare_insert(text);
		if !self.apply_transaction(&tx) {
			return tx;
		}
		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
		tx
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

	/// Prepares paste after cursor, returning transaction and new selection without applying.
	///
	/// Returns None if text is empty.
	pub fn prepare_paste_after(
		&mut self,
		text: &str,
	) -> Option<(Transaction, xeno_base::Selection)> {
		if text.is_empty() {
			return None;
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
						xeno_base::range::Direction::Forward,
						1,
						false,
					)
				})
				.collect()
		};
		self.selection = xeno_base::Selection::from_vec(new_ranges, self.selection.primary_index());
		Some(self.prepare_insert(text))
	}

	/// Pastes text after the cursor position, returning the applied [`Transaction`].
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware paste,
	/// use [`prepare_paste_after`] and apply with [`apply_transaction_with_syntax`].
	pub fn paste_after(&mut self, text: &str) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_paste_after(text)?;
		if !self.apply_transaction(&tx) {
			return None;
		}
		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
		Some(tx)
	}

	/// Prepares paste before cursor, returning transaction and new selection without applying.
	///
	/// Returns None if text is empty.
	pub fn prepare_paste_before(
		&mut self,
		text: &str,
	) -> Option<(Transaction, xeno_base::Selection)> {
		if text.is_empty() {
			return None;
		}
		self.ensure_valid_selection();
		Some(self.prepare_insert(text))
	}

	/// Pastes text before the cursor position, returning the applied [`Transaction`].
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware paste,
	/// use [`prepare_paste_before`] and apply with [`apply_transaction_with_syntax`].
	pub fn paste_before(&mut self, text: &str) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_paste_before(text)?;
		if !self.apply_transaction(&tx) {
			return None;
		}
		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
		Some(tx)
	}

	/// Prepares deletion of selection, returning transaction and new selection without applying.
	///
	/// Returns None if selection is empty.
	pub fn prepare_delete_selection(&mut self) -> Option<(Transaction, xeno_base::Selection)> {
		self.ensure_valid_selection();

		if !self.selection.primary().is_empty() {
			let tx = {
				let doc = self.doc();
				Transaction::delete(doc.content.slice(..), &self.selection)
			};
			let new_selection = tx.map_selection(&self.selection);
			Some((tx, new_selection))
		} else {
			None
		}
	}

	/// Deletes the current selection, returning the applied [`Transaction`] if non-empty.
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware deletion,
	/// use [`prepare_delete_selection`] and apply with [`apply_transaction_with_syntax`].
	pub fn delete_selection(&mut self) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_delete_selection()?;
		if !self.apply_transaction(&tx) {
			return None;
		}
		self.selection = new_selection;
		Some(tx)
	}

	/// Applies a transaction to the document. Increments the version counter.
	///
	/// Returns true if the transaction was applied.
	pub fn apply_transaction(&self, tx: &Transaction) -> bool {
		let mut doc = self.doc_mut();
		if doc.readonly {
			return false;
		}
		tx.apply(&mut doc.content);
		doc.modified = true;
		doc.version = doc.version.wrapping_add(1);
		true
	}

	/// Applies a transaction and updates syntax tree incrementally.
	///
	/// Returns true if the transaction was applied.
	pub fn apply_transaction_with_syntax(
		&self,
		tx: &Transaction,
		language_loader: &LanguageLoader,
	) -> bool {
		let mut doc = self.doc_mut();
		if doc.readonly {
			return false;
		}
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
		true
	}

	/// Finalizes selection/cursor after a transaction is applied.
	pub fn finalize_selection(&mut self, new_selection: xeno_base::Selection) {
		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
	}
}

#[cfg(test)]
mod tests {
	use crate::buffer::{Buffer, BufferId};

	#[test]
	fn readonly_flag_roundtrip() {
		let buffer = Buffer::scratch(BufferId::SCRATCH);
		assert!(!buffer.is_readonly());
		buffer.set_readonly(true);
		assert!(buffer.is_readonly());
	}

	#[test]
	fn readonly_blocks_apply_transaction() {
		let mut buffer = Buffer::scratch(BufferId::SCRATCH);
		let (tx, _selection) = buffer.prepare_insert("hi");
		buffer.set_readonly(true);
		let applied = buffer.apply_transaction(&tx);
		assert!(!applied);
		assert_eq!(buffer.doc().content.slice(..).to_string(), "");
	}
}
