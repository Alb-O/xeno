use super::types::{Bias, Insertion, Operation, Tendril};
use crate::range::{CharIdx, CharLen};
use crate::{Rope, RopeSlice};

/// A sequence of operations representing a set of changes to a document.
///
/// ChangeSet uses Operational Transformation (OT) principles to represent document
/// changes as a sequence of retain, delete, and insert operations. This representation
/// enables efficient composition, inversion, and position mapping.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChangeSet {
	/// Sequence of retain/delete/insert operations.
	pub(super) changes: Vec<Operation>,
	/// Length of the source document before changes.
	pub(super) len: usize,
	/// Length of the document after applying changes.
	pub(super) len_after: usize,
}

impl ChangeSet {
	/// Creates a new empty changeset for the given document.
	///
	/// # Parameters
	/// - `_doc`: The document slice (currently unused, reserved for future validation)
	///
	/// # Returns
	/// An empty [`ChangeSet`].
	pub fn new(_doc: RopeSlice) -> Self {
		Self {
			changes: Vec::new(),
			len: 0,
			len_after: 0,
		}
	}

	/// Returns the length of the source document (before changes).
	pub fn len(&self) -> usize {
		self.len
	}

	/// Returns the length of the document after applying changes.
	pub fn len_after(&self) -> usize {
		self.len_after
	}

	/// Returns true if this changeset contains no operations.
	pub fn is_empty(&self) -> bool {
		self.changes.is_empty()
	}

	/// Returns a slice of all operations in this changeset.
	pub fn changes(&self) -> &[Operation] {
		&self.changes
	}

	/// Adds a retain operation, preserving N characters from the source.
	///
	/// Consecutive retain operations are automatically merged for efficiency.
	///
	/// # Parameters
	/// - `n`: Number of characters to retain
	pub(crate) fn retain(&mut self, n: CharLen) {
		if n == 0 {
			return;
		}

		self.len += n;
		self.len_after += n;

		if let Some(Operation::Retain(count)) = self.changes.last_mut() {
			*count += n;
		} else {
			self.changes.push(Operation::Retain(n));
		}
	}

	/// Adds a delete operation, removing N characters from the source.
	///
	/// Consecutive delete operations are automatically merged for efficiency.
	///
	/// # Parameters
	/// - `n`: Number of characters to delete
	pub(crate) fn delete(&mut self, n: CharLen) {
		if n == 0 {
			return;
		}

		self.len += n;

		if let Some(Operation::Delete(count)) = self.changes.last_mut() {
			*count += n;
		} else {
			self.changes.push(Operation::Delete(n));
		}
	}

	/// Adds an insert operation, inserting text at the current position.
	///
	/// Insert operations are merged with adjacent inserts when possible. The insertion
	/// order relative to deletes is preserved to maintain correct semantics.
	///
	/// # Parameters
	/// - `text`: The text to insert
	pub(crate) fn insert(&mut self, text: Tendril) {
		if text.is_empty() {
			return;
		}

		let ins = Insertion::new(text);
		self.len_after += ins.char_len;

		match self.changes.as_mut_slice() {
			[.., Operation::Insert(prev)] | [.., Operation::Insert(prev), Operation::Delete(_)] => {
				prev.text.push_str(&ins.text);
				prev.char_len += ins.char_len;
			}
			[.., last @ Operation::Delete(_)] => {
				let del = std::mem::replace(last, Operation::Insert(ins));
				self.changes.push(del);
			}
			_ => {
				self.changes.push(Operation::Insert(ins));
			}
		}
	}

	/// Applies this changeset to a document, modifying it in place.
	///
	/// # Parameters
	/// - `doc`: The document to modify
	pub fn apply(&self, doc: &mut Rope) {
		if self.changes.is_empty() {
			return;
		}

		let mut pos = 0;
		for op in &self.changes {
			match op {
				Operation::Retain(n) => {
					pos += n;
				}
				Operation::Delete(n) => {
					doc.remove(pos..pos + n);
				}
				Operation::Insert(ins) => {
					doc.insert(pos, &ins.text);
					pos += ins.char_len;
				}
			}
		}
	}

	/// Inverts this changeset to create one that undoes its effects.
	///
	/// # Parameters
	/// - `doc`: The original document (before changes were applied)
	///
	/// # Returns
	/// A new [`ChangeSet`] that undoes this changeset's changes.
	pub fn invert(&self, doc: &Rope) -> ChangeSet {
		let mut result = ChangeSet {
			changes: Vec::new(),
			len: self.len_after,
			len_after: self.len,
		};

		let mut pos = 0;
		for op in &self.changes {
			match op {
				Operation::Retain(n) => {
					result.retain(*n);
					pos += n;
				}
				Operation::Delete(n) => {
					let deleted_text: String = doc.slice(pos..pos + n).chars().collect();
					result.insert(deleted_text);
					pos += n;
				}
				Operation::Insert(ins) => {
					result.delete(ins.char_len);
				}
			}
		}

		result
	}

	/// Maps a position through this changeset using the specified bias.
	///
	/// # Parameters
	/// - `pos`: The character position to map
	/// - `bias`: How to handle positions at insertion boundaries
	///
	/// # Returns
	/// The mapped position in the transformed document.
	pub fn map_pos(&self, pos: CharIdx, bias: Bias) -> CharIdx {
		let mut old_pos = 0;
		let mut new_pos = 0;

		for op in &self.changes {
			if old_pos > pos {
				break;
			}

			match op {
				Operation::Retain(n) => {
					if old_pos + n > pos {
						return new_pos + (pos - old_pos);
					}
					old_pos += n;
					new_pos += n;
				}
				Operation::Delete(n) => {
					if old_pos + n > pos {
						return new_pos;
					}
					old_pos += n;
				}
				Operation::Insert(ins) => {
					if old_pos == pos && bias == Bias::Left {
						// Position is exactly at insert point, stay before
					} else {
						new_pos += ins.char_len;
					}
				}
			}
		}

		new_pos + (pos - old_pos)
	}

	/// Composes two changesets into a single equivalent changeset.
	///
	/// This implements Operational Transformation composition, allowing multiple
	/// changesets to be combined into a single operation while preserving semantics.
	///
	/// # Parameters
	/// - `other`: The second changeset to compose with this one
	///
	/// # Returns
	/// A new [`ChangeSet`] equivalent to applying `self` then `other`.
	///
	/// # Debug Assertions
	/// Asserts that `self.len_after == other.len` (the changesets must be compatible).
	pub fn compose(self, other: ChangeSet) -> ChangeSet {
		debug_assert_eq!(self.len_after, other.len);

		let mut result = ChangeSet {
			changes: Vec::new(),
			len: self.len,
			len_after: other.len_after,
		};

		let mut a_iter = self.changes.into_iter().peekable();
		let mut b_iter = other.changes.into_iter().peekable();

		let mut a_remaining = 0usize;
		let mut b_remaining = 0usize;

		loop {
			let a = if a_remaining > 0 {
				Some(match a_iter.peek() {
					Some(Operation::Retain(_)) => Operation::Retain(a_remaining),
					Some(Operation::Delete(_)) => Operation::Delete(a_remaining),
					Some(Operation::Insert(ins)) => {
						let text: String = ins.text.chars().take(a_remaining).collect();
						Operation::Insert(Insertion::from_chars(text, a_remaining))
					}
					None => break,
				})
			} else {
				a_iter.next()
			};

			let b = if b_remaining > 0 {
				Some(match b_iter.peek() {
					Some(Operation::Retain(_)) => Operation::Retain(b_remaining),
					Some(Operation::Delete(_)) => Operation::Delete(b_remaining),
					Some(Operation::Insert(ins)) => {
						let text: String = ins.text.chars().take(b_remaining).collect();
						Operation::Insert(Insertion::from_chars(text, b_remaining))
					}
					None => break,
				})
			} else {
				b_iter.next()
			};

			a_remaining = 0;
			b_remaining = 0;

			match (a, b) {
				(None, None) => break,
				(None, Some(Operation::Insert(ins))) => result.insert(ins.text),
				(Some(Operation::Delete(n)), None) => result.delete(n),
				(Some(Operation::Delete(n)), b) => {
					result.delete(n);
					if let Some(op) = b {
						b_remaining = match op {
							Operation::Retain(m) => m,
							Operation::Delete(m) => m,
							Operation::Insert(ins) => ins.char_len,
						};
					}
				}
				(a, Some(Operation::Insert(ins))) => {
					result.insert(ins.text);
					if let Some(op) = a {
						a_remaining = match op {
							Operation::Retain(m) => m,
							Operation::Delete(m) => m,
							Operation::Insert(ins) => ins.char_len,
						};
					}
				}
				(Some(Operation::Retain(n)), Some(Operation::Retain(m))) => {
					let len = n.min(m);
					result.retain(len);
					a_remaining = n - len;
					b_remaining = m - len;
				}
				(Some(Operation::Insert(ins)), Some(Operation::Delete(m))) => {
					let len = ins.char_len.min(m);
					a_remaining = ins.char_len - len;
					b_remaining = m - len;
				}
				(Some(Operation::Insert(ins)), Some(Operation::Retain(m))) => {
					let len = ins.char_len.min(m);
					let text: String = ins.text.chars().take(len).collect();
					result.insert(text);
					a_remaining = ins.char_len - len;
					b_remaining = m - len;
				}
				(Some(Operation::Retain(n)), Some(Operation::Delete(m))) => {
					let len = n.min(m);
					result.delete(len);
					a_remaining = n - len;
					b_remaining = m - len;
				}
				_ => unreachable!(),
			}
		}

		result
	}
}
