//! Core syntax parsing and incremental updates.
//!
//! This module wraps tree-house's Syntax type to provide incremental parsing
//! that integrates with Evildoer's ChangeSet/Transaction system.

use std::ops::RangeBounds;
use std::time::Duration;

use ropey::RopeSlice;
use thiserror::Error;
use tree_house::tree_sitter::{InputEdit, Node, Tree};
use tree_house::{Language, Layer, TreeCursor};

use crate::highlight::Highlighter;
use crate::loader::LanguageLoader;

/// Default parse timeout (500ms).
const PARSE_TIMEOUT: Duration = Duration::from_millis(500);

/// Errors that can occur during syntax operations.
#[derive(Error, Debug)]
pub enum SyntaxError {
	#[error("parse error: {0}")]
	Parse(String),

	#[error("timeout during parsing")]
	Timeout,

	#[error("language not configured")]
	NoLanguage,
}

impl From<tree_house::Error> for SyntaxError {
	fn from(e: tree_house::Error) -> Self {
		SyntaxError::Parse(e.to_string())
	}
}

/// Wrapper around tree-house Syntax for Evildoer integration.
#[derive(Debug)]
pub struct Syntax {
	inner: tree_house::Syntax,
}

impl Syntax {
	/// Creates a new Syntax for the given source and language.
	pub fn new(
		source: RopeSlice,
		language: Language,
		loader: &LanguageLoader,
	) -> Result<Self, SyntaxError> {
		let inner = tree_house::Syntax::new(source, language, PARSE_TIMEOUT, loader)?;
		Ok(Self { inner })
	}

	/// Updates the syntax tree after edits.
	///
	/// This performs incremental re-parsing based on the provided edits.
	pub fn update(
		&mut self,
		source: RopeSlice,
		edits: &[InputEdit],
		loader: &LanguageLoader,
	) -> Result<(), SyntaxError> {
		if edits.is_empty() {
			return Ok(());
		}
		self.inner.update(source, PARSE_TIMEOUT, edits, loader)?;
		Ok(())
	}

	/// Updates from a Evildoer ChangeSet.
	pub fn update_from_changeset(
		&mut self,
		old_source: RopeSlice,
		new_source: RopeSlice,
		changeset: &evildoer_base::ChangeSet,
		loader: &LanguageLoader,
	) -> Result<(), SyntaxError> {
		let edits = generate_edits(old_source, changeset);
		self.update(new_source, &edits, loader)
	}

	/// Returns the root syntax tree.
	pub fn tree(&self) -> &Tree {
		self.inner.tree()
	}

	/// Returns the tree for a specific byte range (may be an injection layer).
	pub fn tree_for_byte_range(&self, start: u32, end: u32) -> &Tree {
		self.inner.tree_for_byte_range(start, end)
	}

	/// Returns the root layer.
	pub fn root_layer(&self) -> Layer {
		self.inner.root()
	}

	/// Returns the language of the root layer.
	pub fn root_language(&self) -> Language {
		self.layer(self.root_layer()).language
	}

	/// Gets data for a specific layer.
	pub fn layer(&self, layer: Layer) -> &tree_house::LayerData {
		self.inner.layer(layer)
	}

	/// Finds the smallest layer containing the byte range.
	pub fn layer_for_byte_range(&self, start: u32, end: u32) -> Layer {
		self.inner.layer_for_byte_range(start, end)
	}

	/// Returns layers containing the byte range, from largest to smallest.
	pub fn layers_for_byte_range(&self, start: u32, end: u32) -> impl Iterator<Item = Layer> + '_ {
		self.inner.layers_for_byte_range(start, end)
	}

	/// Finds the smallest named node containing the byte range.
	pub fn named_descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Node<'_>> {
		self.inner.named_descendant_for_byte_range(start, end)
	}

	/// Finds the smallest node (named or anonymous) containing the byte range.
	pub fn descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Node<'_>> {
		self.inner.descendant_for_byte_range(start, end)
	}

	/// Creates a tree cursor for traversal.
	pub fn walk(&self) -> TreeCursor<'_> {
		self.inner.walk()
	}

	/// Creates a highlighter for the given range.
	pub fn highlighter<'a>(
		&'a self,
		source: RopeSlice<'a>,
		loader: &'a LanguageLoader,
		range: impl RangeBounds<u32>,
	) -> Highlighter<'a> {
		Highlighter::new(&self.inner, source, loader, range)
	}
}

/// Generates tree-sitter InputEdits from a Evildoer ChangeSet.
fn generate_edits(old_text: RopeSlice, changeset: &evildoer_base::ChangeSet) -> Vec<InputEdit> {
	use evildoer_base::transaction::Operation;
	use tree_house::tree_sitter::Point;

	let mut edits = Vec::new();
	let mut old_pos = 0usize;

	if changeset.is_empty() {
		return edits;
	}

	let mut iter = changeset.changes().iter().peekable();

	while let Some(change) = iter.next() {
		let len = match change {
			Operation::Delete(i) | Operation::Retain(i) => *i,
			Operation::Insert(_) => 0,
		};
		let mut old_end = old_pos + len;

		match change {
			Operation::Retain(_) => {}
			Operation::Delete(_) => {
				let start_byte = old_text.char_to_byte(old_pos) as u32;
				let old_end_byte = old_text.char_to_byte(old_end) as u32;

				edits.push(InputEdit {
					start_byte,
					old_end_byte,
					new_end_byte: start_byte,
					start_point: Point::ZERO,
					old_end_point: Point::ZERO,
					new_end_point: Point::ZERO,
				});
			}
			Operation::Insert(s) => {
				let start_byte = old_text.char_to_byte(old_pos) as u32;

				// Check for subsequent delete (replacement)
				if let Some(Operation::Delete(del_len)) = iter.peek() {
					old_end = old_pos + del_len;
					let old_end_byte = old_text.char_to_byte(old_end) as u32;
					iter.next();

					edits.push(InputEdit {
						start_byte,
						old_end_byte,
						new_end_byte: start_byte + s.text.len() as u32,
						start_point: Point::ZERO,
						old_end_point: Point::ZERO,
						new_end_point: Point::ZERO,
					});
				} else {
					edits.push(InputEdit {
						start_byte,
						old_end_byte: start_byte,
						new_end_byte: start_byte + s.text.len() as u32,
						start_point: Point::ZERO,
						old_end_point: Point::ZERO,
						new_end_point: Point::ZERO,
					});
				}
			}
		}
		old_pos = old_end;
	}

	edits
}

/// Pretty-prints a syntax tree node for debugging.
pub fn pretty_print_tree<W: std::fmt::Write>(fmt: &mut W, node: Node) -> std::fmt::Result {
	if node.child_count() == 0 {
		if node.is_named() {
			write!(fmt, "({})", node.kind())
		} else {
			write!(
				fmt,
				"\"{}\"",
				node.kind().replace('\\', "\\\\").replace('"', "\\\"")
			)
		}
	} else {
		pretty_print_tree_impl(fmt, &mut node.walk(), 0)
	}
}

fn pretty_print_tree_impl<W: std::fmt::Write>(
	fmt: &mut W,
	cursor: &mut tree_house::tree_sitter::TreeCursor,
	depth: usize,
) -> std::fmt::Result {
	let node = cursor.node();
	let visible = node.is_missing()
		|| (node.is_named() && node.grammar().node_kind_is_visible(node.kind_id()));

	if visible {
		let indent = depth * 2;
		write!(fmt, "{:indent$}", "")?;

		if let Some(field) = cursor.field_name() {
			write!(fmt, "{}: ", field)?;
		}

		write!(fmt, "({}", node.kind())?;
	} else {
		write!(
			fmt,
			" \"{}\"",
			node.kind().replace('\\', "\\\\").replace('"', "\\\"")
		)?;
	}

	if cursor.goto_first_child() {
		loop {
			if cursor.node().is_named() || cursor.node().is_missing() {
				fmt.write_char('\n')?;
			}

			pretty_print_tree_impl(fmt, cursor, depth + 1)?;

			if !cursor.goto_next_sibling() {
				break;
			}
		}
		cursor.goto_parent();
	}

	if visible {
		fmt.write_char(')')?;
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use evildoer_base::{Rope, Transaction};

	use super::*;

	#[test]
	fn test_generate_edits_insert() {
		use evildoer_base::transaction::Change;
		let doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 5,
			end: 5,
			replacement: Some(" beautiful".into()),
		}];
		let tx = Transaction::change(doc.slice(..), changes.into_iter());

		let edits = generate_edits(doc.slice(..), tx.changes());
		assert_eq!(edits.len(), 1);
		assert_eq!(edits[0].start_byte, 5);
		assert_eq!(edits[0].old_end_byte, 5);
		assert_eq!(edits[0].new_end_byte, 15);
	}

	#[test]
	fn test_generate_edits_delete() {
		use evildoer_base::transaction::Change;
		let doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 5,
			end: 11,
			replacement: None,
		}];
		let tx = Transaction::change(doc.slice(..), changes.into_iter());

		let edits = generate_edits(doc.slice(..), tx.changes());
		assert_eq!(edits.len(), 1);
		assert_eq!(edits[0].start_byte, 5);
		assert_eq!(edits[0].old_end_byte, 11);
		assert_eq!(edits[0].new_end_byte, 5);
	}

	#[test]
	fn test_generate_edits_replace() {
		use evildoer_base::transaction::Change;
		let doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 6,
			end: 11,
			replacement: Some("rust".into()),
		}];
		let tx = Transaction::change(doc.slice(..), changes.into_iter());

		let edits = generate_edits(doc.slice(..), tx.changes());
		assert_eq!(edits.len(), 1);
		assert_eq!(edits[0].start_byte, 6);
		assert_eq!(edits[0].old_end_byte, 11);
		assert_eq!(edits[0].new_end_byte, 10);
	}
}
