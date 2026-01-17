//! Text editing operations for buffers.

#[cfg(feature = "lsp")]
use xeno_lsp::{IncrementalResult, OffsetEncoding, compute_lsp_changes};
#[cfg(feature = "lsp")]
use xeno_primitives::LspDocumentChange;
use xeno_primitives::{CommitResult, EditCommit, Range, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_runtime_language::LanguageLoader;

/// Policy for applying a transaction to a buffer.
///
/// Combines undo and syntax policies. Use builder methods to configure,
/// or the predefined constants for common cases.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyPolicy {
	/// How to handle undo recording.
	pub undo: UndoPolicy,
	/// How to handle syntax tree updates.
	pub syntax: SyntaxPolicy,
}

impl ApplyPolicy {
	/// No undo recording, no syntax update. For internal operations.
	pub const BARE: Self = Self {
		undo: UndoPolicy::NoUndo,
		syntax: SyntaxPolicy::None,
	};

	/// Record undo, incremental syntax update. Standard edit policy.
	pub const EDIT: Self = Self {
		undo: UndoPolicy::Record,
		syntax: SyntaxPolicy::IncrementalOrDirty,
	};

	/// Merge with current undo group, incremental syntax. For insert-mode.
	pub const INSERT: Self = Self {
		undo: UndoPolicy::MergeWithCurrentGroup,
		syntax: SyntaxPolicy::IncrementalOrDirty,
	};

	/// Sets the undo policy.
	pub const fn with_undo(mut self, undo: UndoPolicy) -> Self {
		self.undo = undo;
		self
	}

	/// Sets the syntax policy.
	pub const fn with_syntax(mut self, syntax: SyntaxPolicy) -> Self {
		self.syntax = syntax;
		self
	}
}

use super::Buffer;
use crate::movement;

impl Buffer {
	/// Inserts text at all cursor positions, returning the [`Transaction`] without applying it.
	///
	/// The caller is responsible for applying the transaction (with or without syntax update).
	pub fn prepare_insert(&mut self, text: &str) -> (Transaction, xeno_primitives::Selection) {
		self.ensure_valid_selection();

		let mut insertion_points = self.selection.clone();
		insertion_points.transform_mut(|r| *r = Range::point(r.head));

		let tx = self.with_doc(|doc| {
			Transaction::insert(doc.content().slice(..), &insertion_points, text.to_string())
		});
		let new_selection = tx.map_selection(&self.selection);

		(tx, new_selection)
	}

	/// Inserts text at all cursor positions, returning the applied [`Transaction`].
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware insertion,
	/// use [`prepare_insert`] and apply with [`apply`] using `ApplyPolicy::EDIT`.
	pub fn insert_text(&mut self, text: &str) -> Transaction {
		let (tx, new_selection) = self.prepare_insert(text);
		let result = self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		if !result.applied {
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
			let text = self.with_doc(|doc| doc.content().slice(from..to).to_string());
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
	) -> Option<(Transaction, xeno_primitives::Selection)> {
		if text.is_empty() {
			return None;
		}
		self.ensure_valid_selection();

		// Compute new ranges by moving each cursor forward by 1
		let new_ranges: Vec<_> = self.with_doc(|doc| {
			self.selection
				.ranges()
				.iter()
				.map(|r| {
					movement::move_horizontally(
						doc.content().slice(..),
						*r,
						xeno_primitives::range::Direction::Forward,
						1,
						false,
					)
				})
				.collect()
		});
		self.set_selection(xeno_primitives::Selection::from_vec(
			new_ranges,
			self.selection.primary_index(),
		));
		Some(self.prepare_insert(text))
	}

	/// Pastes text after the cursor position, returning the applied [`Transaction`].
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware paste,
	/// use [`prepare_paste_after`] and apply with [`apply`] using `ApplyPolicy::EDIT`.
	pub fn paste_after(&mut self, text: &str) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_paste_after(text)?;
		let result = self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		if !result.applied {
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
	) -> Option<(Transaction, xeno_primitives::Selection)> {
		if text.is_empty() {
			return None;
		}
		self.ensure_valid_selection();
		Some(self.prepare_insert(text))
	}

	/// Pastes text before the cursor position, returning the applied [`Transaction`].
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware paste,
	/// use [`prepare_paste_before`] and apply with [`apply`] using `ApplyPolicy::EDIT`.
	pub fn paste_before(&mut self, text: &str) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_paste_before(text)?;
		let result = self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		if !result.applied {
			return None;
		}
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
		Some(tx)
	}

	/// Prepares deletion of selection, returning transaction and new selection without applying.
	///
	/// Returns None if selection is empty.
	pub fn prepare_delete_selection(
		&mut self,
	) -> Option<(Transaction, xeno_primitives::Selection)> {
		self.ensure_valid_selection();

		if !self.selection.primary().is_empty() {
			let tx =
				self.with_doc(|doc| Transaction::delete(doc.content().slice(..), &self.selection));
			let new_selection = tx.map_selection(&self.selection);
			Some((tx, new_selection))
		} else {
			None
		}
	}

	/// Deletes the current selection, returning the applied [`Transaction`] if non-empty.
	///
	/// Note: This does NOT update syntax highlighting. For syntax-aware deletion,
	/// use [`prepare_delete_selection`] and apply with [`apply`] using `ApplyPolicy::EDIT`.
	pub fn delete_selection(&mut self) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_delete_selection()?;
		let result = self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		if !result.applied {
			return None;
		}
		self.set_selection(new_selection);
		Some(tx)
	}

	/// Applies a transaction with the specified policy.
	///
	/// This is the unified entry point for applying transactions. Use [`ApplyPolicy`]
	/// constants or builder methods to configure undo and syntax behavior.
	///
	/// Returns a [`CommitResult`] with `applied=false` if the buffer is read-only.
	///
	/// # Examples
	///
	/// ```ignore
	/// // Standard edit with undo recording
	/// buffer.apply(&tx, ApplyPolicy::EDIT, &loader);
	///
	/// // Insert-mode edit (merges with current undo group)
	/// buffer.apply(&tx, ApplyPolicy::INSERT, &loader);
	///
	/// // Custom policy
	/// buffer.apply(&tx, ApplyPolicy::BARE.with_undo(UndoPolicy::Record), &loader);
	/// ```
	pub fn apply(
		&self,
		tx: &Transaction,
		policy: ApplyPolicy,
		loader: &LanguageLoader,
	) -> CommitResult {
		if self.readonly_override == Some(true) {
			return self
				.with_doc(|doc| CommitResult::blocked(doc.version(), doc.insert_undo_active()));
		}
		if self.readonly_override.is_none() {
			let (readonly, version, insert_active) =
				self.with_doc(|doc| (doc.is_readonly(), doc.version(), doc.insert_undo_active()));
			if readonly {
				return CommitResult::blocked(version, insert_active);
			}
		}

		let commit = EditCommit::new(tx.clone())
			.with_undo(policy.undo)
			.with_syntax(policy.syntax);

		self.with_doc_mut(|doc| doc.commit_unchecked(commit, loader))
	}

	/// Applies a transaction with LSP change tracking.
	///
	/// Like [`apply`], but also computes and queues LSP document changes for
	/// incremental sync. The encoding specifies how to compute LSP positions.
	///
	/// Returns a [`CommitResult`] with `applied=false` if the buffer is read-only.
	#[cfg(feature = "lsp")]
	pub fn apply_with_lsp(
		&self,
		tx: &Transaction,
		policy: ApplyPolicy,
		loader: &LanguageLoader,
		encoding: OffsetEncoding,
	) -> CommitResult {
		if self.readonly_override == Some(true) {
			return self
				.with_doc(|doc| CommitResult::blocked(doc.version(), doc.insert_undo_active()));
		}
		if self.readonly_override.is_none() {
			let (readonly, version, insert_active) =
				self.with_doc(|doc| (doc.is_readonly(), doc.version(), doc.insert_undo_active()));
			if readonly {
				return CommitResult::blocked(version, insert_active);
			}
		}

		// Compute LSP changes before applying the transaction (needs pre-edit state)
		let lsp_changes = self.with_doc(|doc| compute_lsp_changes(doc.content(), tx, encoding));

		let commit = EditCommit::new(tx.clone())
			.with_undo(policy.undo)
			.with_syntax(policy.syntax);

		self.with_doc_mut(|doc| {
			let result = doc.commit_unchecked(commit, loader);

			match lsp_changes {
				IncrementalResult::Incremental(changes) => {
					if !changes.is_empty() {
						doc.extend_lsp_changes(changes);
					}
				}
				IncrementalResult::FallbackToFull => {
					doc.mark_for_full_lsp_sync();
				}
			}
			result
		})
	}

	/// Drains pending LSP changes for this document.
	#[cfg(feature = "lsp")]
	pub fn drain_lsp_changes(&self) -> Vec<LspDocumentChange> {
		self.with_doc_mut(|doc| doc.drain_lsp_changes())
	}

	/// Finalizes selection/cursor after a transaction is applied.
	pub fn finalize_selection(&mut self, new_selection: xeno_primitives::Selection) {
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
	}
}

#[cfg(test)]
mod tests {
	use xeno_runtime_language::LanguageLoader;

	use crate::buffer::{ApplyPolicy, Buffer, ViewId};

	#[cfg(feature = "lsp")]
	mod lsp_batching {
		use xeno_lsp::{IncrementalResult, OffsetEncoding, compute_lsp_changes};
		use xeno_primitives::lsp::{LspPosition, LspRange};
		use xeno_primitives::{Selection, SyntaxPolicy};
		use xeno_runtime_language::LanguageLoader;

		use crate::buffer::{ApplyPolicy, Buffer, ViewId};

		/// Policy shorthand for LSP-synced edits without undo.
		const LSP_POLICY: ApplyPolicy =
			ApplyPolicy::BARE.with_syntax(SyntaxPolicy::IncrementalOrDirty);

		fn make_buffer(content: &str) -> Buffer {
			let buffer = Buffer::scratch(ViewId::SCRATCH);
			if !content.is_empty() {
				let rope = ropey::Rope::from(content);
				buffer.with_doc_mut(|doc| doc.reset_content(rope));
			}
			buffer
		}

		#[test]
		fn single_insert_queues_one_change() {
			let mut buffer = make_buffer("hello");
			buffer.set_selection(Selection::single(5, 5)); // cursor at end

			let (tx, _sel) = buffer.prepare_insert(" world");
			let loader = xeno_runtime_language::LanguageLoader::new();
			buffer.apply_with_lsp(&tx, LSP_POLICY, &loader, OffsetEncoding::Utf16);

			let changes = buffer.drain_lsp_changes();
			assert_eq!(changes.len(), 1);
			assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 5)));
			assert_eq!(changes[0].new_text, " world");
		}

		#[test]
		fn multiple_transactions_accumulate_changes() {
			let mut buffer = make_buffer("line1\nline2\n");
			let loader = xeno_runtime_language::LanguageLoader::new();

			// First transaction: insert at start of line 1
			buffer.set_selection(Selection::single(0, 0));
			let (tx1, sel1) = buffer.prepare_insert("A");
			buffer.apply_with_lsp(&tx1, LSP_POLICY, &loader, OffsetEncoding::Utf16);
			buffer.finalize_selection(sel1);

			// Second transaction: insert at start of line 2
			// After first insert, "Aline1\nline2\n", line 2 starts at char 7
			buffer.set_selection(Selection::single(7, 7));
			let (tx2, sel2) = buffer.prepare_insert("B");
			buffer.apply_with_lsp(&tx2, LSP_POLICY, &loader, OffsetEncoding::Utf16);
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
			let loader = xeno_runtime_language::LanguageLoader::new();

			// Multi-cursor: start of each line
			buffer.set_selection(Selection::from_vec(
				vec![
					xeno_primitives::Range::point(0),
					xeno_primitives::Range::point(4),
					xeno_primitives::Range::point(8),
				],
				0,
			));

			let (tx, _sel) = buffer.prepare_insert("X");
			buffer.apply_with_lsp(&tx, LSP_POLICY, &loader, OffsetEncoding::Utf16);

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
		fn incremental_changes_match_reference() {
			let mut buffer = make_buffer("hello");
			buffer.set_selection(Selection::single(5, 5));

			let (tx, new_sel) = buffer.prepare_insert("!");
			let expected = buffer
				.with_doc(|doc| compute_lsp_changes(doc.content(), &tx, OffsetEncoding::Utf16));

			let loader = LanguageLoader::new();
			let result = buffer.apply_with_lsp(
				&tx,
				ApplyPolicy::BARE.with_syntax(SyntaxPolicy::IncrementalOrDirty),
				&loader,
				OffsetEncoding::Utf16,
			);
			assert!(result.applied);
			buffer.finalize_selection(new_sel);

			match expected {
				IncrementalResult::Incremental(changes) => {
					let actual = buffer.drain_lsp_changes();
					assert_eq!(actual, changes);
					assert!(!buffer.with_doc(|doc| doc.needs_full_lsp_sync()));
				}
				IncrementalResult::FallbackToFull => {
					let actual = buffer.drain_lsp_changes();
					assert!(actual.is_empty());
					assert!(buffer.with_doc(|doc| doc.needs_full_lsp_sync()));
				}
			}
		}

		#[test]
		fn drain_clears_pending_changes() {
			let mut buffer = make_buffer("test");
			let loader = xeno_runtime_language::LanguageLoader::new();

			buffer.set_selection(Selection::single(4, 4));
			let (tx, _sel) = buffer.prepare_insert("!");
			buffer.apply_with_lsp(&tx, LSP_POLICY, &loader, OffsetEncoding::Utf16);

			let changes = buffer.drain_lsp_changes();
			assert_eq!(changes.len(), 1);

			// Second drain should be empty
			let changes2 = buffer.drain_lsp_changes();
			assert!(changes2.is_empty());
		}
	}

	#[test]
	fn readonly_flag_roundtrip() {
		let buffer = Buffer::scratch(ViewId::SCRATCH);
		assert!(!buffer.is_readonly());
		buffer.set_readonly(true);
		assert!(buffer.is_readonly());
	}

	#[test]
	fn readonly_blocks_apply_transaction() {
		let mut buffer = Buffer::scratch(ViewId::SCRATCH);
		let (tx, _selection) = buffer.prepare_insert("hi");
		buffer.set_readonly(true);
		let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		assert!(!result.applied);
		assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "");
	}

	#[test]
	fn readonly_override_blocks_transaction() {
		let mut buffer = Buffer::scratch(ViewId::SCRATCH);
		assert!(!buffer.with_doc(|doc| doc.is_readonly()));
		buffer.set_readonly_override(Some(true));
		assert!(buffer.is_readonly());

		let (tx, _selection) = buffer.prepare_insert("hi");
		let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		assert!(!result.applied);
		assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "");
	}

	#[test]
	fn readonly_override_allows_write_on_readonly_doc() {
		let mut buffer = Buffer::scratch(ViewId::SCRATCH);
		// Document is readonly, but buffer override makes it writable
		buffer.set_readonly(true);
		assert!(buffer.is_readonly());

		buffer.set_readonly_override(Some(false));
		assert!(!buffer.is_readonly());

		let (tx, _selection) = buffer.prepare_insert("hi");
		let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
		assert!(result.applied);
		assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "hi");
	}

	#[test]
	fn readonly_override_none_defers_to_document() {
		let mut buffer = Buffer::scratch(ViewId::SCRATCH);
		buffer.set_readonly_override(None);
		assert!(!buffer.is_readonly()); // Document is writable

		buffer.set_readonly(true);
		assert!(buffer.is_readonly()); // Now document is readonly, override defers
	}

	#[test]
	fn split_does_not_inherit_readonly_override() {
		let mut buffer = Buffer::scratch(ViewId::SCRATCH);
		buffer.set_readonly_override(Some(true));
		assert!(buffer.is_readonly());

		let split = buffer.clone_for_split(ViewId(1));
		// Split should defer to document (writable), not inherit override
		assert!(!split.is_readonly());
	}
}
