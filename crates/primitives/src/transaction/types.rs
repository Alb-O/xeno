use crate::range::{CharIdx, CharLen};

/// A tendril is a reference-counted string slice used for efficient text operations.
///
/// Currently aliased to [`String`] for simplicity. Future implementations may use
/// a more sophisticated rope-based or reference-counted structure.
pub type Tendril = String;

/// Represents a single text change operation.
///
/// A change describes replacing the text range `[start, end)` with the optional
/// `replacement` text. If `replacement` is [`None`], this represents a deletion.
pub struct Change {
	/// The starting character index of the change.
	pub start: CharIdx,
	/// The ending character index of the change (exclusive).
	pub end: CharIdx,
	/// The replacement text, or [`None`] for deletion.
	pub replacement: Option<Tendril>,
}

/// Bias determines how positions at change boundaries are mapped.
///
/// When mapping a position through a change, bias determines whether the position
/// moves with insertions or stays before them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bias {
	/// Position stays before insertions at the same location.
	Left,
	/// Position moves after insertions at the same location.
	Right,
}

/// A text insertion with cached character length.
///
/// Storing the character count avoids repeated O(n) `.chars().count()` calls
/// in hot paths like `apply()`, `map_pos()`, and `compose()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insertion {
	/// The text to insert.
	pub text: Tendril,
	/// Cached character count of `text`.
	pub char_len: CharLen,
}

impl Insertion {
	/// Creates a new insertion, computing the character length once.
	///
	/// # Parameters
	/// - `text`: The text to insert
	///
	/// # Returns
	/// A new [`Insertion`] with pre-computed character length.
	#[inline]
	pub fn new(text: Tendril) -> Self {
		let char_len = text.chars().count();
		Self { text, char_len }
	}

	/// Creates an insertion from a substring with pre-computed length.
	///
	/// # Parameters
	/// - `text`: The text to insert
	/// - `char_len`: Pre-computed character count (must match actual count)
	///
	/// # Returns
	/// A new [`Insertion`] using the provided length.
	///
	/// # Debug Assertions
	/// In debug builds, asserts that `char_len` matches the actual character count.
	#[inline]
	pub fn from_chars(text: Tendril, char_len: CharLen) -> Self {
		debug_assert_eq!(text.chars().count(), char_len);
		Self { text, char_len }
	}

	/// Returns true if this insertion is empty.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.char_len == 0
	}
}

/// A single operation in a changeset.
///
/// Operations are the atomic units that make up a `ChangeSet`. They represent
/// basic text transformations: retaining existing text, deleting text, or inserting
/// new text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
	/// Retain the next N characters from the source document.
	Retain(CharLen),
	/// Delete the next N characters from the source document.
	Delete(CharLen),
	/// Insert new text at the current position.
	Insert(Insertion),
}
