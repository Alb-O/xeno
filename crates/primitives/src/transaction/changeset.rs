use super::types::{Bias, Insertion, Operation, Tendril};
use crate::range::{CharIdx, CharLen};
use crate::{Rope, RopeSlice};

/// A sequence of operations representing a set of changes to a document.
///
/// ChangeSet uses Operational Transformation (OT) principles to represent document
/// changes as a sequence of retain, delete, and insert operations. This representation
/// enables efficient composition, inversion, and position mapping.
///
/// # Invariants
///
/// - The sum of `retain` and `delete` lengths must equal `len` (input length).
/// - The sum of `retain` and `insert` lengths must equal `len_after` (output length).
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
	/// Creates a new identity changeset for the given document.
	///
	/// An identity changeset has `len == len_after == doc.len_chars()` and
	/// represents no changes.
	pub fn new(doc: RopeSlice) -> Self {
		let n = doc.len_chars();
		let mut cs = Self::builder();
		if n > 0 {
			cs.retain(n);
		}
		cs
	}

	/// Creates an empty builder with zero length.
	///
	/// Use this when building a changeset from scratch using `retain`, `delete`, and `insert`.
	pub(crate) fn builder() -> Self {
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

	/// Returns true if this changeset represents an identity transformation.
	///
	/// An identity transformation only contains Retain operations and does
	/// not change the document content or length.
	pub fn is_identity(&self) -> bool {
		self.changes.iter().all(|op| matches!(op, Operation::Retain(_)))
	}

	/// Returns a slice of all operations in this changeset.
	pub fn changes(&self) -> &[Operation] {
		&self.changes
	}

	/// Adds a retain operation, preserving N characters from the source.
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
	pub(crate) fn insert(&mut self, text: Tendril) {
		if text.is_empty() {
			return;
		}
		let ins = Insertion::new(text);
		self.len_after += ins.char_len();

		// Coalesce with adjacent insertions or swap with trailing delete
		// to maintain canonical form (Insert before Delete).
		match self.changes.as_mut_slice() {
			[.., Operation::Insert(prev)] | [.., Operation::Insert(prev), Operation::Delete(_)] => {
				prev.push_str(&ins);
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
	pub fn apply(&self, doc: &mut Rope) {
		let mut pos = 0;
		for op in &self.changes {
			match op {
				Operation::Retain(n) => pos += n,
				Operation::Delete(n) => doc.remove(pos..pos + n),
				Operation::Insert(ins) => {
					doc.insert(pos, ins.text());
					pos += ins.char_len();
				}
			}
		}
	}

	/// Inverts this changeset to create one that undoes its effects.
	///
	/// # Arguments
	///
	/// * `doc` - The document state *before* this changeset was applied.
	///   Required to recover deleted text.
	pub fn invert(&self, doc: &Rope) -> ChangeSet {
		let mut result = Self::builder();
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
					result.delete(ins.char_len());
				}
			}
		}
		debug_assert_eq!(result.len, self.len_after);
		debug_assert_eq!(result.len_after, self.len);
		#[cfg(debug_assertions)]
		result.debug_assert_consistent();
		result
	}

	/// Maps a position through this changeset.
	///
	/// # Arguments
	///
	/// * `pos` - The position in the pre-change document.
	/// * `bias` - Determines behavior at insertion boundaries:
	///   - `Bias::Left`: Stick to the character before the insertion (stay put).
	///   - `Bias::Right`: Stick to the character after the insertion (move with inserted text).
	pub fn map_pos(&self, pos: CharIdx, bias: Bias) -> CharIdx {
		let mut old_pos = 0;
		let mut new_pos = 0;
		for op in &self.changes {
			match op {
				Operation::Insert(ins) => {
					if old_pos == pos && bias == Bias::Left {
						return new_pos;
					}
					new_pos += ins.char_len();
				}
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
			}
		}
		new_pos + pos.saturating_sub(old_pos)
	}

	/// Composes two changesets into a single equivalent changeset.
	///
	/// `self` must apply to document A to produce B.
	/// `other` must apply to document B to produce C.
	/// Result applies to A to produce C.
	///
	/// # Panics
	///
	/// Panics if `self.len_after() != other.len()`.
	pub fn compose(self, other: ChangeSet) -> ChangeSet {
		assert_eq!(
			self.len_after, other.len,
			"composition mismatch: first changeset output length ({}) != \
			 second changeset input length ({})",
			self.len_after, other.len
		);
		let mut result = Self::builder();
		let mut a_iter = self.changes.into_iter();
		let mut b_iter = other.changes.into_iter();
		let mut a_op = a_iter.next();
		let mut b_op = b_iter.next();

		loop {
			match (a_op.take(), b_op.take()) {
				(None, None) => break,
				(Some(Operation::Delete(n)), b) => {
					result.delete(n);
					b_op = b;
				}
				(a, Some(Operation::Insert(ins))) => {
					result.insert(ins.into_text());
					a_op = a;
				}
				(Some(Operation::Retain(n)), Some(Operation::Retain(m))) => {
					let len = n.min(m);
					result.retain(len);
					a_op = (n > len).then(|| Operation::Retain(n - len));
					b_op = (m > len).then(|| Operation::Retain(m - len));
				}
				(Some(Operation::Insert(mut ins)), Some(Operation::Delete(m))) => {
					let len = ins.char_len().min(m);
					if ins.char_len() > len {
						let _ = ins.take_prefix(len);
						a_op = Some(Operation::Insert(ins));
					}
					if m > len {
						b_op = Some(Operation::Delete(m - len));
					}
				}
				(Some(Operation::Insert(ins)), Some(Operation::Retain(m))) => {
					let len = ins.char_len().min(m);
					let (prefix, suffix_ins) = ins.split_at(len);
					result.insert(prefix);
					if suffix_ins.char_len() > 0 {
						a_op = Some(Operation::Insert(suffix_ins));
					}
					if m > len {
						b_op = Some(Operation::Retain(m - len));
					}
				}
				(Some(Operation::Retain(n)), Some(Operation::Delete(m))) => {
					let len = n.min(m);
					result.delete(len);
					if n > len {
						a_op = Some(Operation::Retain(n - len));
					}
					if m > len {
						b_op = Some(Operation::Delete(m - len));
					}
				}
				(None, Some(op)) => {
					match op {
						Operation::Insert(ins) => result.insert(ins.into_text()),
						_ => unreachable!("invalid composition: extra op in second changeset"),
					}
					b_op = b_iter.next();
				}
				(Some(op), None) => {
					match op {
						Operation::Delete(n) => result.delete(n),
						_ => unreachable!("invalid composition: extra op in first changeset"),
					}
					a_op = a_iter.next();
				}
			}

			if a_op.is_none() {
				a_op = a_iter.next();
			}
			if b_op.is_none() {
				b_op = b_iter.next();
			}
		}

		debug_assert_eq!(result.len, self.len);
		debug_assert_eq!(result.len_after, other.len_after);
		#[cfg(debug_assertions)]
		result.debug_assert_consistent();
		result
	}

	#[cfg(debug_assertions)]
	fn debug_assert_consistent(&self) {
		let mut in_len = 0usize;
		let mut out_len = 0usize;
		for op in &self.changes {
			match op {
				Operation::Retain(n) => {
					in_len += n;
					out_len += n;
				}
				Operation::Delete(n) => in_len += n,
				Operation::Insert(ins) => out_len += ins.char_len(),
			}
		}
		debug_assert_eq!(in_len, self.len, "Input length mismatch");
		debug_assert_eq!(out_len, self.len_after, "Output length mismatch");
	}
}
