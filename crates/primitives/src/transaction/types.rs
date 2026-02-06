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
#[derive(Debug, Clone)]
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
///
/// Fields are private to enforce the invariant that `char_len` always equals
/// `text.chars().count()`. Construct via [`Insertion::new`] or
/// [`Insertion::from_chars`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insertion {
	text: Tendril,
	char_len: CharLen,
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

	/// Returns the inserted text.
	#[inline]
	pub fn text(&self) -> &str {
		&self.text
	}

	/// Returns the cached character length.
	#[inline]
	pub fn char_len(&self) -> CharLen {
		self.char_len
	}

	/// Returns the byte length of the inserted text.
	#[inline]
	pub fn byte_len(&self) -> usize {
		self.text.len()
	}

	/// Appends text from another insertion, updating the cached length.
	pub(super) fn push_str(&mut self, other: &Insertion) {
		self.text.push_str(&other.text);
		self.char_len += other.char_len;
	}

	/// Consumes this insertion and returns the owned text.
	pub(super) fn into_text(self) -> Tendril {
		self.text
	}

	/// Splits off the first `n` characters, returning them as a new string.
	///
	/// The remaining insertion has its char_len reduced accordingly.
	pub(super) fn take_prefix(&mut self, n: CharLen) -> Tendril {
		debug_assert!(n <= self.char_len);
		let text: String = self.text.chars().take(n).collect();
		let rest: String = self.text.chars().skip(n).collect();
		self.text = rest;
		self.char_len -= n;
		text
	}

	/// Splits off everything after the first `n` characters, returning a new Insertion.
	///
	/// `self` is consumed and the caller gets the suffix.
	pub(super) fn split_at(self, n: CharLen) -> (Tendril, Insertion) {
		debug_assert!(n <= self.char_len);
		let prefix: String = self.text.chars().take(n).collect();
		let suffix: String = self.text.chars().skip(n).collect();
		let suffix_ins = Insertion::from_chars(suffix, self.char_len - n);
		(prefix, suffix_ins)
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
