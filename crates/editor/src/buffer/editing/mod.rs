//! Text editing operations for buffers.

use xeno_primitives::{CommitResult, EditCommit, Range, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_runtime_language::LanguageLoader;

use crate::types::Yank;

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

#[cfg(test)]
mod tests;

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

	/// Yanks the current selections to the yank register.
	///
	/// In the 1-cell minimum model, this preserves each selection fragment as a separate
	/// entry in the [`Yank`] payload. Point selections (cursors) yank exactly one character
	/// (the character cell they occupy).
	///
	/// # Returns
	///
	/// Returns `Some(Yank)` containing the fragments and total count, or `None` if the
	/// document is empty.
	pub fn yank_selection(&mut self) -> Option<Yank> {
		self.ensure_valid_selection();

		self.with_doc(|doc| {
			let text = doc.content().slice(..);
			let len = text.len_chars();

			let mut parts = Vec::new();
			let mut total_chars = 0;

			for range in self.selection.ranges() {
				let (from, to) = range.extent_clamped(len);
				if from < to {
					let fragment = text.slice(from..to).to_string();
					total_chars += fragment.chars().count();
					parts.push(fragment);
				}
			}

			if !parts.is_empty() {
				Some(Yank { parts, total_chars })
			} else {
				None
			}
		})
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

		let tx = self.with_doc(|doc| Transaction::delete(doc.content().slice(..), &self.selection));
		let new_selection = tx.map_selection(&self.selection);
		Some((tx, new_selection))
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

	/// Applies a remote sync transaction, bypassing the buffer-level readonly
	/// override.
	///
	/// Follower buffers have `readonly_override = Some(true)` to block user
	/// edits, but remote deltas from the owner must still be applied. Only
	/// checks the document-level readonly flag (underlying file permissions).
	pub fn apply_remote(
		&self,
		tx: &Transaction,
		policy: ApplyPolicy,
		loader: &LanguageLoader,
	) -> CommitResult {
		let (readonly, version, insert_active) =
			self.with_doc(|doc| (doc.is_readonly(), doc.version(), doc.insert_undo_active()));
		if readonly {
			return CommitResult::blocked(version, insert_active);
		}

		let commit = EditCommit::new(tx.clone())
			.with_undo(policy.undo)
			.with_syntax(policy.syntax);

		self.with_doc_mut(|doc| doc.commit_unchecked(commit, loader))
	}

	/// Finalizes selection/cursor after a transaction is applied.
	pub fn finalize_selection(&mut self, new_selection: xeno_primitives::Selection) {
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
	}
}
