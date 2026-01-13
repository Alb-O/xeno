//! Text editing operations for buffers.

#[cfg(feature = "lsp")]
use xeno_base::LspDocumentChange;
use xeno_base::{Range, Transaction};
use xeno_core::movement;
use xeno_language::LanguageLoader;
#[cfg(feature = "lsp")]
use xeno_lsp::{IncrementalResult, OffsetEncoding, compute_lsp_changes};

use super::Buffer;

impl Buffer {
	/// Inserts text at all cursor positions, returning the [`Transaction`] without applying it.
	///
	/// The caller is responsible for applying the transaction (with or without syntax update).
	pub fn prepare_insert(&mut self, text: &str) -> (Transaction, xeno_base::Selection) {
		self.ensure_valid_selection();

		let mut insertion_points = self.selection.clone();
		insertion_points.transform_mut(|r| *r = Range::point(r.head));

		let tx = {
			let doc = self.doc();
			Transaction::insert(doc.content.slice(..), &insertion_points, text.to_string())
		};
		let new_selection = tx.map_selection(&self.selection);

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
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
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
		self.set_selection(xeno_base::Selection::from_vec(
			new_ranges,
			self.selection.primary_index(),
		));
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
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
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
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
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
		self.set_selection(new_selection);
		Some(tx)
	}

	/// Applies a transaction to the document. Increments the version counter.
	///
	/// Returns true if the transaction was applied. Returns false if the buffer
	/// is read-only (either via buffer override or document flag).
	pub fn apply_transaction(&self, tx: &Transaction) -> bool {
		if self.readonly_override == Some(true) {
			return false;
		}
		let mut doc = self.doc_mut();
		if self.readonly_override.is_none() && doc.readonly {
			return false;
		}
		tx.apply(&mut doc.content);
		doc.modified = true;
		doc.version = doc.version.wrapping_add(1);
		true
	}

	/// Applies a transaction and updates syntax tree incrementally.
	///
	/// Returns true if the transaction was applied. Returns false if the buffer
	/// is read-only (either via buffer override or document flag).
	pub fn apply_transaction_with_syntax(
		&self,
		tx: &Transaction,
		language_loader: &LanguageLoader,
	) -> bool {
		if self.readonly_override == Some(true) {
			return false;
		}
		let mut doc = self.doc_mut();
		if self.readonly_override.is_none() && doc.readonly {
			return false;
		}
		let old_doc = doc.content.clone();
		tx.apply(&mut doc.content);

		if doc.syntax.is_some() {
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

	/// Applies a transaction, updates syntax, and queues LSP changes.
	#[cfg(feature = "lsp")]
	pub fn apply_edit_with_lsp(
		&self,
		tx: &Transaction,
		language_loader: &LanguageLoader,
		encoding: OffsetEncoding,
	) -> bool {
		if self.readonly_override == Some(true) {
			return false;
		}
		let mut doc = self.doc_mut();
		if self.readonly_override.is_none() && doc.readonly {
			return false;
		}

		let old_doc = doc.content.clone();
		let lsp_changes = compute_lsp_changes(&old_doc, tx, encoding);
		tx.apply(&mut doc.content);

		if doc.syntax.is_some() {
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
		match lsp_changes {
			IncrementalResult::Incremental(changes) => {
				if !changes.is_empty() {
					doc.pending_lsp_changes.extend(changes);
				}
			}
			IncrementalResult::FallbackToFull => {
				doc.pending_lsp_changes.clear();
				doc.force_full_sync = true;
			}
		}
		true
	}

	/// Drains pending LSP changes for this document.
	#[cfg(feature = "lsp")]
	pub fn drain_lsp_changes(&self) -> Vec<LspDocumentChange> {
		std::mem::take(&mut self.doc_mut().pending_lsp_changes)
	}

	/// Finalizes selection/cursor after a transaction is applied.
	pub fn finalize_selection(&mut self, new_selection: xeno_base::Selection) {
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
	}
}

#[cfg(test)]
mod tests {
	use crate::buffer::{Buffer, BufferId};

	#[cfg(feature = "lsp")]
	mod lsp_batching {
		use xeno_base::Selection;
		use xeno_base::lsp::{LspPosition, LspRange};
		use xeno_lsp::OffsetEncoding;

		use crate::buffer::{Buffer, BufferId};

		fn make_buffer(content: &str) -> Buffer {
			let buffer = Buffer::scratch(BufferId::SCRATCH);
			if !content.is_empty() {
				// Set initial content by replacing the empty document
				let rope = ropey::Rope::from(content);
				buffer.doc_mut().content = rope;
			}
			buffer
		}

		#[test]
		fn single_insert_queues_one_change() {
			let mut buffer = make_buffer("hello");
			buffer.set_selection(Selection::single(5, 5)); // cursor at end

			let (tx, _sel) = buffer.prepare_insert(" world");
			let loader = xeno_language::LanguageLoader::new();
			buffer.apply_edit_with_lsp(&tx, &loader, OffsetEncoding::Utf16);

			let changes = buffer.drain_lsp_changes();
			assert_eq!(changes.len(), 1);
			assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 5)));
			assert_eq!(changes[0].new_text, " world");
		}

		#[test]
		fn multiple_transactions_accumulate_changes() {
			let mut buffer = make_buffer("line1\nline2\n");
			let loader = xeno_language::LanguageLoader::new();

			// First transaction: insert at start of line 1
			buffer.set_selection(Selection::single(0, 0));
			let (tx1, sel1) = buffer.prepare_insert("A");
			buffer.apply_edit_with_lsp(&tx1, &loader, OffsetEncoding::Utf16);
			buffer.finalize_selection(sel1);

			// Second transaction: insert at start of line 2
			// After first insert, "Aline1\nline2\n", line 2 starts at char 7
			buffer.set_selection(Selection::single(7, 7));
			let (tx2, sel2) = buffer.prepare_insert("B");
			buffer.apply_edit_with_lsp(&tx2, &loader, OffsetEncoding::Utf16);
			buffer.finalize_selection(sel2);

			let changes = buffer.drain_lsp_changes();
			assert_eq!(changes.len(), 2);

			// First change: insert "A" at (0, 0) in original doc
			assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 0)));
			assert_eq!(changes[0].new_text, "A");

			// Second change: insert "B" at (1, 0) in doc after first change
			// The position is computed against the state AFTER first transaction
			assert_eq!(changes[1].range, LspRange::point(LspPosition::new(1, 0)));
			assert_eq!(changes[1].new_text, "B");
		}

		#[test]
		fn multi_cursor_single_transaction_queues_ordered_changes() {
			let mut buffer = make_buffer("aaa\nbbb\nccc\n");
			let loader = xeno_language::LanguageLoader::new();

			// Multi-cursor: start of each line
			buffer.set_selection(Selection::from_vec(
				vec![
					xeno_base::Range::point(0),
					xeno_base::Range::point(4),
					xeno_base::Range::point(8),
				],
				0,
			));

			let (tx, _sel) = buffer.prepare_insert("X");
			buffer.apply_edit_with_lsp(&tx, &loader, OffsetEncoding::Utf16);

			let changes = buffer.drain_lsp_changes();
			assert_eq!(changes.len(), 3);

			// Changes are ordered by position in pre-change document,
			// but positions are computed as transaction is applied
			assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 0)));
			assert_eq!(changes[0].new_text, "X");

			// After first insert: "Xaaa\n..." - second cursor was at char 4,
			// but in scratch rope it's at original position since we track shifts
			assert_eq!(changes[1].range, LspRange::point(LspPosition::new(1, 0)));
			assert_eq!(changes[1].new_text, "X");

			assert_eq!(changes[2].range, LspRange::point(LspPosition::new(2, 0)));
			assert_eq!(changes[2].new_text, "X");
		}

		#[test]
		fn drain_clears_pending_changes() {
			let mut buffer = make_buffer("test");
			let loader = xeno_language::LanguageLoader::new();

			buffer.set_selection(Selection::single(4, 4));
			let (tx, _sel) = buffer.prepare_insert("!");
			buffer.apply_edit_with_lsp(&tx, &loader, OffsetEncoding::Utf16);

			let changes = buffer.drain_lsp_changes();
			assert_eq!(changes.len(), 1);

			// Second drain should be empty
			let changes2 = buffer.drain_lsp_changes();
			assert!(changes2.is_empty());
		}
	}

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

	#[test]
	fn readonly_override_blocks_transaction() {
		let mut buffer = Buffer::scratch(BufferId::SCRATCH);
		// Document is writable, but buffer override makes it readonly
		assert!(!buffer.doc().readonly);
		buffer.set_readonly_override(Some(true));
		assert!(buffer.is_readonly());

		let (tx, _selection) = buffer.prepare_insert("hi");
		let applied = buffer.apply_transaction(&tx);
		assert!(!applied);
		assert_eq!(buffer.doc().content.slice(..).to_string(), "");
	}

	#[test]
	fn readonly_override_allows_write_on_readonly_doc() {
		let mut buffer = Buffer::scratch(BufferId::SCRATCH);
		// Document is readonly, but buffer override makes it writable
		buffer.set_readonly(true);
		assert!(buffer.is_readonly());

		buffer.set_readonly_override(Some(false));
		assert!(!buffer.is_readonly());

		let (tx, _selection) = buffer.prepare_insert("hi");
		let applied = buffer.apply_transaction(&tx);
		assert!(applied);
		assert_eq!(buffer.doc().content.slice(..).to_string(), "hi");
	}

	#[test]
	fn readonly_override_none_defers_to_document() {
		let mut buffer = Buffer::scratch(BufferId::SCRATCH);
		buffer.set_readonly_override(None);
		assert!(!buffer.is_readonly()); // Document is writable

		buffer.set_readonly(true);
		assert!(buffer.is_readonly()); // Now document is readonly, override defers
	}

	#[test]
	fn split_does_not_inherit_readonly_override() {
		let mut buffer = Buffer::scratch(BufferId::SCRATCH);
		buffer.set_readonly_override(Some(true));
		assert!(buffer.is_readonly());

		let split = buffer.clone_for_split(BufferId(1));
		// Split should defer to document (writable), not inherit override
		assert!(!split.is_readonly());
	}
}
