mod changeset;
#[cfg(test)]
mod tests;
mod types;

pub use changeset::ChangeSet;
pub use types::{Bias, Change, Insertion, Operation, Tendril};

use crate::range::CharIdx;
use crate::{Range, Rope, RopeSlice, Selection};

/// A document transformation combining changes with optional selection updates.
///
/// Transaction wraps a [`ChangeSet`] with an optional [`Selection`], providing
/// a high-level API for common editing operations like insert, delete, and change.
/// Transactions can be inverted for undo/redo and composed for efficient batching.
#[derive(Debug, Clone)]
pub struct Transaction {
	changes: ChangeSet,
	selection: Option<Selection>,
}

impl Transaction {
	/// Creates a new empty transaction for the given document.
	///
	/// # Parameters
	/// - `doc`: The document slice
	///
	/// # Returns
	/// An empty [`Transaction`] with no changes or selection.
	pub fn new(doc: RopeSlice) -> Self {
		Self {
			changes: ChangeSet::new(doc),
			selection: None,
		}
	}

	/// Creates a transaction from an iterator of changes.
	///
	/// Changes must be non-overlapping and sorted by start position. This function
	/// converts the high-level change representation into a low-level [`ChangeSet`].
	///
	/// # Parameters
	/// - `doc`: The document slice
	/// - `changes`: Iterator of non-overlapping, sorted changes
	///
	/// # Returns
	/// A new [`Transaction`] representing the changes.
	pub fn change<I>(doc: RopeSlice, changes: I) -> Self
	where
		I: IntoIterator<Item = Change>,
	{
		let mut changeset = ChangeSet::new(doc);
		let mut last: CharIdx = 0;

		for change in changes {
			let from = change.start;
			let to = change.end;
			let replacement = change.replacement;
			debug_assert!(from <= to);
			debug_assert!(from >= last);

			if from > last {
				changeset.retain(from - last);
			}

			if to > from {
				changeset.delete(to - from);
			}

			if let Some(text) = replacement {
				changeset.insert(text);
			}

			last = to;
		}

		let remaining = doc.len_chars() - last;
		if remaining > 0 {
			changeset.retain(remaining);
		}

		Self {
			changes: changeset,
			selection: None,
		}
	}

	/// Creates a transaction that inserts text at each selection range.
	///
	/// For each range in the selection, replaces the range `[min, max)` with the
	/// provided text. This enables multi-cursor editing.
	///
	/// # Parameters
	/// - `doc`: The document slice
	/// - `selection`: The ranges where text should be inserted
	/// - `text`: The text to insert at each range
	///
	/// # Returns
	/// A new [`Transaction`] representing the insertions.
	pub fn insert(doc: RopeSlice, selection: &Selection, text: Tendril) -> Self {
		Self::change(
			doc,
			selection.iter().map(|r: &Range| Change {
				start: r.min(),
				end: r.max(),
				replacement: Some(text.clone()),
			}),
		)
	}

	/// Creates a transaction that deletes each selection range.
	///
	/// For each range in the selection, deletes the text in `[min, max)`.
	///
	/// # Parameters
	/// - `doc`: The document slice
	/// - `selection`: The ranges to delete
	///
	/// # Returns
	/// A new [`Transaction`] representing the deletions.
	pub fn delete(doc: RopeSlice, selection: &Selection) -> Self {
		Self::change(
			doc,
			selection.iter().map(|r: &Range| Change {
				start: r.min(),
				end: r.max(),
				replacement: None,
			}),
		)
	}

	/// Attaches a selection to this transaction.
	///
	/// The selection will be returned when the transaction is applied.
	///
	/// # Parameters
	/// - `selection`: The selection to attach
	///
	/// # Returns
	/// This transaction with the selection attached.
	pub fn with_selection(mut self, selection: Selection) -> Self {
		self.selection = Some(selection);
		self
	}

	/// Returns a reference to this transaction's changeset.
	pub fn changes(&self) -> &ChangeSet {
		&self.changes
	}

	/// Returns a reference to this transaction's selection, if any.
	pub fn selection(&self) -> Option<&Selection> {
		self.selection.as_ref()
	}

	/// Applies this transaction to a document, modifying it in place.
	///
	/// # Parameters
	/// - `doc`: The document to modify
	///
	/// # Returns
	/// The attached selection, if any.
	pub fn apply(&self, doc: &mut Rope) -> Option<Selection> {
		self.changes.apply(doc);
		self.selection.clone()
	}

	/// Creates a transaction that undoes this one.
	///
	/// # Parameters
	/// - `doc`: The original document (before this transaction was applied)
	///
	/// # Returns
	/// A new [`Transaction`] that undoes this transaction's changes.
	pub fn invert(&self, doc: &Rope) -> Self {
		Self {
			changes: self.changes.invert(doc),
			selection: None,
		}
	}

	/// Maps a selection through this transaction's changes.
	///
	/// Transforms each range in the selection by mapping its anchor and head
	/// positions through the changeset, preserving the range direction.
	///
	/// # Parameters
	/// - `selection`: The selection to transform
	///
	/// # Returns
	/// A new [`Selection`] with ranges mapped through this transaction.
	pub fn map_selection(&self, selection: &Selection) -> Selection {
		selection.transform(|range| {
			Range::new(
				self.changes.map_pos(range.anchor, Bias::Left),
				self.changes.map_pos(range.head, Bias::Right),
			)
		})
	}
}
