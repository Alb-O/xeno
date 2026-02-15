//! Transaction model for document edits.
//!
//! Wraps low-level change operations with optional selection updates and
//! provides apply/invert/mapping helpers used by undo/redo and edit commands.

/// Text change set implementation.
mod changeset;
#[cfg(test)]
mod tests;
/// Transaction primitive types.
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
	/// The underlying set of document changes.
	changes: ChangeSet,
	/// Optional selection update to apply after changes.
	selection: Option<Selection>,
}

impl Transaction {
	/// Creates a new empty transaction for the given document.
	///
	/// # Returns
	///
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
	/// # Panics
	///
	/// Panics if changes are overlapping or out of order.
	pub fn change<I>(doc: RopeSlice, changes: I) -> Self
	where
		I: IntoIterator<Item = Change>,
	{
		let mut changeset = ChangeSet::builder();
		let mut last: CharIdx = 0;
		let doc_len = doc.len_chars();

		for change in changes {
			let from = change.start;
			let to = change.end.min(doc_len);
			let replacement = change.replacement;
			assert!(from <= doc_len, "change start ({from}) exceeds document length ({doc_len})");
			assert!(from <= to, "change start ({from}) exceeds change end ({to})");
			assert!(from >= last, "changes overlap or are out of order: start ({from}) < previous end ({last})");

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

		let remaining = doc_len.checked_sub(last).expect("last position exceeds document length");
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
	/// In the 1-cell minimum model, this inserts text before each range's
	/// start position. This enables multi-cursor editing while preserving
	/// the cell under the cursor.
	pub fn insert(doc: RopeSlice, selection: &Selection, text: Tendril) -> Self {
		Self::change(
			doc,
			selection.iter().map(|r: &Range| Change {
				start: r.from(),
				end: r.from(),
				replacement: Some(text.clone()),
			}),
		)
	}

	/// Creates a transaction that deletes each selection range.
	///
	/// For each range in the selection, deletes the text in `[from, to)`.
	pub fn delete(doc: RopeSlice, selection: &Selection) -> Self {
		let len = doc.len_chars();
		Self::change(
			doc,
			selection.iter().map(|r: &Range| Change {
				start: r.from(),
				end: r.to().min(len),
				replacement: None,
			}),
		)
	}

	/// Attaches a selection to this transaction.
	///
	/// The selection will be returned when the transaction is applied.
	pub fn with_selection(mut self, selection: Selection) -> Self {
		self.selection = Some(selection);
		self
	}

	/// Returns a reference to this transaction's changeset.
	pub fn changes(&self) -> &ChangeSet {
		&self.changes
	}

	/// Returns the underlying operation list for this transaction.
	pub fn operations(&self) -> &[Operation] {
		self.changes.changes()
	}

	/// Returns a reference to this transaction's selection, if any.
	pub fn selection(&self) -> Option<&Selection> {
		self.selection.as_ref()
	}

	/// Returns true if this transaction represents an identity transformation.
	pub fn is_identity(&self) -> bool {
		self.changes.is_identity()
	}

	/// Applies this transaction to a document, modifying it in place.
	///
	/// # Returns
	///
	/// The attached selection, if any.
	pub fn apply(&self, doc: &mut Rope) -> Option<Selection> {
		self.changes.apply(doc);
		self.selection.clone()
	}

	/// Creates a transaction that undoes this one.
	///
	/// # Arguments
	///
	/// * `doc` - The original document (before this transaction was applied).
	pub fn invert(&self, doc: &Rope) -> Self {
		Self {
			changes: self.changes.invert(doc),
			selection: None,
		}
	}

	/// Maps a selection through this transaction's changes using 1-cell model semantics.
	///
	/// Maps the extent boundaries `[from, to)` with biases that preserve half-open
	/// interval invariants, then reconstructs the inclusive cell range.
	///
	/// # Semantics
	///
	/// * `from` maps with `Bias::Right`: cursor follows insertions at the boundary.
	/// * `to` maps with `Bias::Left`: end boundary doesn't expand on insertion.
	pub fn map_selection(&self, selection: &Selection) -> Selection {
		selection.transform(|r| {
			let dir = r.direction();
			let from = r.from();
			let to = r.to();

			// Map boundaries:
			// * start boundary from maps with Bias::Right (insertions shift it right)
			// * end boundary to maps with Bias::Left (insertions do not expand it)
			let from2 = self.changes.map_pos(from, Bias::Right);
			let to2 = self.changes.map_pos(to, Bias::Left);

			let (lo, hi) = if to2 <= from2 {
				(from2, from2) // Collapsed to 1-cell point
			} else {
				(from2, to2 - 1) // Map back to inclusive cell head
			};

			match dir {
				crate::range::Direction::Forward => Range::new(lo, hi),
				crate::range::Direction::Backward => Range::new(hi, lo),
			}
		})
	}
}
