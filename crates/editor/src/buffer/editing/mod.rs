//! Text editing operations for buffers.

use xeno_primitives::{CommitResult, EditCommit, Range, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_runtime_language::LanguageLoader;

use crate::types::Yank;

/// Application policy for document transactions.
///
/// Combines undo and syntax policies to define how a modification should be
/// integrated into the editor state.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyPolicy {
	/// History recording policy for this transaction.
	pub undo: UndoPolicy,
	/// Syntax update policy for this transaction.
	pub syntax: SyntaxPolicy,
}

impl ApplyPolicy {
	/// Transaction applied without history recording or syntax updates.
	pub const BARE: Self = Self {
		undo: UndoPolicy::NoUndo,
		syntax: SyntaxPolicy::None,
	};

	/// Standard edit with history recording and incremental syntax updates.
	pub const EDIT: Self = Self {
		undo: UndoPolicy::Record,
		syntax: SyntaxPolicy::IncrementalOrDirty,
	};

	/// Insert-mode edit that merges with the active undo group.
	pub const INSERT: Self = Self {
		undo: UndoPolicy::MergeWithCurrentGroup,
		syntax: SyntaxPolicy::IncrementalOrDirty,
	};

	/// Returns a copy of the policy with the specified [`UndoPolicy`].
	pub const fn with_undo(mut self, undo: UndoPolicy) -> Self {
		self.undo = undo;
		self
	}

	/// Returns a copy of the policy with the specified [`SyntaxPolicy`].
	pub const fn with_syntax(mut self, syntax: SyntaxPolicy) -> Self {
		self.syntax = syntax;
		self
	}
}

use xeno_input::movement;

use super::Buffer;

#[cfg(test)]
mod tests;

impl Buffer {
	/// Prepares an insertion transaction at all cursor positions.
	///
	/// Returns the [`Transaction`] and the resulting [`Selection`] without
	/// applying them.
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

	/// Inserts text at all cursor positions.
	///
	/// # Note
	///
	/// This does NOT update syntax highlighting. For syntax-aware insertion,
	/// apply the transaction via [`apply`] with [`ApplyPolicy::EDIT`].
	pub fn insert_text(&mut self, text: &str) -> Transaction {
		let (tx, new_selection) = self.prepare_insert(text);
		if self
			.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new())
			.applied
		{
			self.set_selection(new_selection);
			self.sync_cursor_to_selection();
		}
		tx
	}

	/// Yanks the current selection(s) to a [`Yank`] payload.
	///
	/// Preserves each selection fragment as a separate entry. Point selections
	/// yank exactly one character.
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

			(!parts.is_empty()).then_some(Yank { parts, total_chars })
		})
	}

	/// Prepares a paste operation after each cursor.
	pub fn prepare_paste_after(
		&mut self,
		text: &str,
	) -> Option<(Transaction, xeno_primitives::Selection)> {
		(!text.is_empty()).then(|| {
			self.ensure_valid_selection();
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
			self.prepare_insert(text)
		})
	}

	/// Pastes text after the cursor positions.
	pub fn paste_after(&mut self, text: &str) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_paste_after(text)?;
		self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new())
			.applied
			.then(|| {
				self.set_selection(new_selection);
				self.sync_cursor_to_selection();
				tx
			})
	}

	/// Prepares a paste operation before each cursor.
	pub fn prepare_paste_before(
		&mut self,
		text: &str,
	) -> Option<(Transaction, xeno_primitives::Selection)> {
		(!text.is_empty()).then(|| {
			self.ensure_valid_selection();
			self.prepare_insert(text)
		})
	}

	/// Pastes text before the cursor positions.
	pub fn paste_before(&mut self, text: &str) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_paste_before(text)?;
		self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new())
			.applied
			.then(|| {
				self.set_selection(new_selection);
				self.sync_cursor_to_selection();
				tx
			})
	}

	/// Prepares deletion of the current selection.
	pub fn prepare_delete_selection(
		&mut self,
	) -> Option<(Transaction, xeno_primitives::Selection)> {
		self.ensure_valid_selection();
		let tx = self.with_doc(|doc| Transaction::delete(doc.content().slice(..), &self.selection));
		let new_selection = tx.map_selection(&self.selection);
		Some((tx, new_selection))
	}

	/// Deletes the current selection.
	pub fn delete_selection(&mut self) -> Option<Transaction> {
		let (tx, new_selection) = self.prepare_delete_selection()?;
		self.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new())
			.applied
			.then(|| {
				self.set_selection(new_selection);
				tx
			})
	}

	/// Applies a transaction with the specified policy.
	///
	/// This is the unified entry point for all local document modifications.
	/// It resolves undo grouping and enforces view-level readonly checks.
	pub fn apply(
		&mut self,
		tx: &Transaction,
		policy: ApplyPolicy,
		loader: &LanguageLoader,
	) -> CommitResult {
		if let Some(readonly) = self.readonly_override {
			if readonly {
				return CommitResult::blocked(self.version());
			}
		} else if self.with_doc(|doc| doc.is_readonly()) {
			return CommitResult::blocked(self.version());
		}

		let merge = match policy.undo {
			UndoPolicy::MergeWithCurrentGroup => self.insert_undo_active,
			_ => false,
		};

		let commit = EditCommit::new(tx.clone())
			.with_undo(policy.undo)
			.with_syntax(policy.syntax);

		let result = self.with_doc_mut(|doc| doc.commit_unchecked(commit, merge, loader));

		if result.applied {
			self.insert_undo_active = matches!(policy.undo, UndoPolicy::MergeWithCurrentGroup);
		}

		result
	}

	/// Applies a remote transaction, bypassing view-level readonly overrides.
	///
	/// Remote edits always clear the local insert-undo group to maintain
	/// history consistency.
	pub fn apply_remote(
		&mut self,
		tx: &Transaction,
		policy: ApplyPolicy,
		loader: &LanguageLoader,
	) -> CommitResult {
		if self.with_doc(|doc| doc.is_readonly()) {
			return CommitResult::blocked(self.version());
		}

		let commit = EditCommit::new(tx.clone())
			.with_undo(policy.undo)
			.with_syntax(policy.syntax);

		let result = self.with_doc_mut(|doc| doc.commit_unchecked(commit, false, loader));
		self.insert_undo_active = false;
		result
	}

	/// Finalizes view state (selection and cursor) after a successful edit.
	pub fn finalize_selection(&mut self, new_selection: xeno_primitives::Selection) {
		self.set_selection(new_selection);
		self.sync_cursor_to_selection();
	}
}
