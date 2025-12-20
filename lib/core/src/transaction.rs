use crate::range::{CharIdx, CharLen};
use crate::{Range, Rope, RopeSlice, Selection};

pub type Tendril = String;

pub struct Change {
	pub start: CharIdx,
	pub end: CharIdx,
	pub replacement: Option<Tendril>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bias {
	Left,
	Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
	Retain(CharLen),
	Delete(CharLen),
	Insert(Tendril),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChangeSet {
	changes: Vec<Operation>,
	len: usize,
	len_after: usize,
}

impl ChangeSet {
	pub fn new(_doc: RopeSlice) -> Self {
		Self {
			changes: Vec::new(),
			len: 0,
			len_after: 0,
		}
	}

	pub fn len(&self) -> usize {
		self.len
	}

	pub fn len_after(&self) -> usize {
		self.len_after
	}

	pub fn is_empty(&self) -> bool {
		self.changes.is_empty()
	}

	pub fn changes(&self) -> &[Operation] {
		&self.changes
	}

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

	pub(crate) fn insert(&mut self, text: Tendril) {
		if text.is_empty() {
			return;
		}

		self.len_after += text.chars().count();

		match self.changes.as_mut_slice() {
			[.., Operation::Insert(prev)] | [.., Operation::Insert(prev), Operation::Delete(_)] => {
				prev.push_str(&text);
			}
			[.., last @ Operation::Delete(_)] => {
				let del = std::mem::replace(last, Operation::Insert(text));
				self.changes.push(del);
			}
			_ => {
				self.changes.push(Operation::Insert(text));
			}
		}
	}

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
				Operation::Insert(text) => {
					doc.insert(pos, text);
					pos += text.chars().count();
				}
			}
		}
	}

	/// Invert this changeset to create one that undoes its effects.
	/// Must be called with the original document (before apply).
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
					// To undo a delete, we insert what was deleted
					let deleted_text: String = doc.slice(pos..pos + n).chars().collect();
					result.insert(deleted_text);
					pos += n;
				}
				Operation::Insert(text) => {
					// To undo an insert, we delete what was inserted
					result.delete(text.chars().count());
				}
			}
		}

		result
	}

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
				Operation::Insert(text) => {
					let len = text.chars().count();
					if old_pos == pos && bias == Bias::Left {
						// Position is exactly at insert point, stay before
					} else {
						new_pos += len;
					}
				}
			}
		}

		new_pos + (pos - old_pos)
	}

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
					Some(Operation::Insert(t)) => {
						Operation::Insert(t.chars().take(a_remaining).collect())
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
					Some(Operation::Insert(t)) => {
						Operation::Insert(t.chars().take(b_remaining).collect())
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
				(None, Some(Operation::Insert(t))) => result.insert(t),
				(Some(Operation::Delete(n)), None) => result.delete(n),
				(Some(Operation::Delete(n)), b) => {
					result.delete(n);
					if let Some(op) = b {
						b_remaining = match op {
							Operation::Retain(m) => m,
							Operation::Delete(m) => m,
							Operation::Insert(t) => t.chars().count(),
						};
					}
				}
				(a, Some(Operation::Insert(t))) => {
					result.insert(t);
					if let Some(op) = a {
						a_remaining = match op {
							Operation::Retain(m) => m,
							Operation::Delete(m) => m,
							Operation::Insert(t) => t.chars().count(),
						};
					}
				}
				(Some(Operation::Retain(n)), Some(Operation::Retain(m))) => {
					let len = n.min(m);
					result.retain(len);
					a_remaining = n - len;
					b_remaining = m - len;
				}
				(Some(Operation::Insert(t)), Some(Operation::Delete(m))) => {
					let len = t.chars().count().min(m);
					a_remaining = t.chars().count() - len;
					b_remaining = m - len;
				}
				(Some(Operation::Insert(t)), Some(Operation::Retain(m))) => {
					let len = t.chars().count().min(m);
					result.insert(t.chars().take(len).collect());
					a_remaining = t.chars().count() - len;
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

#[derive(Debug, Clone)]
pub struct Transaction {
	changes: ChangeSet,
	selection: Option<Selection>,
}

impl Transaction {
	pub fn new(doc: RopeSlice) -> Self {
		Self {
			changes: ChangeSet::new(doc),
			selection: None,
		}
	}

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

	pub fn with_selection(mut self, selection: Selection) -> Self {
		self.selection = Some(selection);
		self
	}

	pub fn changes(&self) -> &ChangeSet {
		&self.changes
	}

	pub fn selection(&self) -> Option<&Selection> {
		self.selection.as_ref()
	}

	pub fn apply(&self, doc: &mut Rope) -> Option<Selection> {
		self.changes.apply(doc);
		self.selection.clone()
	}

	/// Create a transaction that undoes this one.
	/// Must be called with the original document (before apply).
	pub fn invert(&self, doc: &Rope) -> Self {
		Self {
			changes: self.changes.invert(doc),
			selection: None,
		}
	}

	pub fn map_selection(&self, selection: &Selection) -> Selection {
		selection.transform(|range| {
			Range::new(
				self.changes.map_pos(range.anchor, Bias::Left),
				self.changes.map_pos(range.head, Bias::Right),
			)
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_changeset_retain() {
		let doc = Rope::from("hello");
		let mut cs = ChangeSet::new(doc.slice(..));
		cs.retain(5);
		assert_eq!(cs.len(), 5);
		assert_eq!(cs.len_after(), 5);
	}

	#[test]
	fn test_changeset_delete() {
		let doc = Rope::from("hello");
		let mut cs = ChangeSet::new(doc.slice(..));
		cs.delete(2);
		cs.retain(3);
		assert_eq!(cs.len(), 5);
		assert_eq!(cs.len_after(), 3);
	}

	#[test]
	fn test_changeset_insert() {
		let doc = Rope::from("hello");
		let mut cs = ChangeSet::new(doc.slice(..));
		cs.insert("world".into());
		cs.retain(5);
		assert_eq!(cs.len(), 5);
		assert_eq!(cs.len_after(), 10);
	}

	#[test]
	fn test_changeset_apply() {
		let mut doc = Rope::from("hello");
		let mut cs = ChangeSet::new(doc.slice(..));
		cs.delete(2);
		cs.insert("aa".into());
		cs.retain(3);
		cs.apply(&mut doc);
		assert_eq!(doc.to_string(), "aallo");
	}

	#[test]
	fn test_transaction_insert() {
		let mut doc = Rope::from("hello world");
		let sel = Selection::single(5, 5);
		let tx = Transaction::insert(doc.slice(..), &sel, ",".into());
		tx.apply(&mut doc);
		assert_eq!(doc.to_string(), "hello, world");
	}

	#[test]
	fn test_transaction_delete() {
		let mut doc = Rope::from("hello world");
		let sel = Selection::single(5, 6);
		let tx = Transaction::delete(doc.slice(..), &sel);
		tx.apply(&mut doc);
		assert_eq!(doc.to_string(), "helloworld");
	}

	#[test]
	fn test_transaction_change() {
		let mut doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 0,
			end: 5,
			replacement: Some("hi".into()),
		}];
		let tx = Transaction::change(doc.slice(..), changes);
		tx.apply(&mut doc);
		assert_eq!(doc.to_string(), "hi world");
	}

	#[test]
	fn test_map_selection() {
		let doc = Rope::from("hello world");
		let sel = Selection::single(6, 11);
		let tx = Transaction::change(
			doc.slice(..),
			vec![Change {
				start: 0,
				end: 0,
				replacement: Some("!! ".into()),
			}],
		);
		let mapped = tx.map_selection(&sel);
		assert_eq!(mapped.primary().anchor, 9);
		assert_eq!(mapped.primary().head, 14);
	}
}
