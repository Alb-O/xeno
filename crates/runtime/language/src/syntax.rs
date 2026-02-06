//! Core syntax parsing and incremental updates.
//!
//! This module wraps tree-house's Syntax type to provide incremental parsing
//! that integrates with Xeno's ChangeSet/Transaction system.

use std::ops::RangeBounds;
use std::time::Duration;

use ropey::RopeSlice;
use thiserror::Error;
use tree_house::tree_sitter::{InputEdit, Node, Tree};
use tree_house::{Language, Layer, TreeCursor};

use crate::highlight::Highlighter;
use crate::loader::LanguageLoader;

/// Default parse timeout (500ms).
const DEFAULT_PARSE_TIMEOUT: Duration = Duration::from_millis(500);

/// Injection handling policy.
///
/// NOTE: actual injection enable/disable is implemented by `LanguageLoader`
/// (tree-house queries + injected language resolution). `SyntaxOptions` just
/// plumbs the intent through the call chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPolicy {
	/// Build all injection layers (current behavior).
	Eager,
	/// Disable injection layers entirely (root language only).
	Disabled,
}

/// Options controlling a (re)parse.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxOptions {
	pub parse_timeout: Duration,
	pub injections: InjectionPolicy,
}

impl Default for SyntaxOptions {
	fn default() -> Self {
		Self {
			parse_timeout: DEFAULT_PARSE_TIMEOUT,
			injections: InjectionPolicy::Eager,
		}
	}
}

/// Errors that can occur during syntax operations.
#[derive(Error, Debug)]
pub enum SyntaxError {
	/// Tree-sitter parsing failed.
	#[error("parse error: {0}")]
	Parse(String),

	/// Parsing took longer than the configured timeout.
	#[error("timeout during parsing")]
	Timeout,

	/// No language configuration found for the file type.
	#[error("language not configured")]
	NoLanguage,
}

impl From<tree_house::Error> for SyntaxError {
	fn from(e: tree_house::Error) -> Self {
		match e {
			tree_house::Error::Timeout => SyntaxError::Timeout,
			other => SyntaxError::Parse(other.to_string()),
		}
	}
}

/// Wrapper around tree-house Syntax for Xeno integration.
#[derive(Debug, Clone)]
pub struct Syntax {
	/// The underlying tree-house syntax tree.
	inner: tree_house::Syntax,
	/// Options used for the current parse.
	opts: SyntaxOptions,
}

impl Syntax {
	/// Creates a new syntax tree with the given options.
	pub fn new(
		source: RopeSlice,
		language: Language,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Self, SyntaxError> {
		let loader = loader.with_injections(matches!(opts.injections, InjectionPolicy::Eager));
		let inner = tree_house::Syntax::new(source, language, opts.parse_timeout, &loader)?;
		Ok(Self { inner, opts })
	}

	/// Updates the syntax tree after edits (incremental) with the given options.
	pub fn update(
		&mut self,
		source: RopeSlice,
		edits: &[InputEdit],
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<(), SyntaxError> {
		if edits.is_empty() {
			return Ok(());
		}
		self.opts = opts;
		let loader = loader.with_injections(matches!(opts.injections, InjectionPolicy::Eager));
		self.inner
			.update(source, opts.parse_timeout, edits, &loader)?;
		Ok(())
	}

	/// Updates from a Xeno ChangeSet with the given options.
	pub fn update_from_changeset(
		&mut self,
		old_source: RopeSlice,
		new_source: RopeSlice,
		changeset: &xeno_primitives::ChangeSet,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<(), SyntaxError> {
		let edits = generate_edits(old_source, changeset);
		self.update(new_source, &edits, loader, opts)
	}

	/// Returns the options used for the current parse.
	pub fn opts(&self) -> SyntaxOptions {
		self.opts
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

/// Generates tree-sitter InputEdits from a Xeno ChangeSet.
fn generate_edits(old_text: RopeSlice, changeset: &xeno_primitives::ChangeSet) -> Vec<InputEdit> {
	use tree_house::tree_sitter::Point;
	use xeno_primitives::transaction::Operation;

	fn add_delta(start: Point, text: &str) -> Point {
		let bytes = text.as_bytes();
		let mut row = start.row;
		let mut col = start.col;
		for &b in bytes {
			if b == b'\n' {
				row += 1;
				col = 0;
			} else {
				col += 1;
			}
		}
		Point { row, col }
	}

	fn add_delta_rope(start: Point, rope: RopeSlice) -> Point {
		let mut p = start;
		for chunk in rope.chunks() {
			p = add_delta(p, chunk);
		}
		p
	}

	let mut edits = Vec::new();
	let mut old_pos = 0usize;
	let mut current_byte = 0u32;
	let mut current_point = Point { row: 0, col: 0 };

	if changeset.is_empty() {
		return edits;
	}

	let mut iter = changeset.changes().iter().peekable();

	while let Some(change) = iter.next() {
		match change {
			Operation::Retain(len) => {
				let segment = old_text.slice(old_pos..old_pos + len);
				current_byte += segment.len_bytes() as u32;
				current_point = add_delta_rope(current_point, segment);
				old_pos += len;
			}
			Operation::Delete(len) => {
				let start_byte = current_byte;
				let start_point = current_point;

				let segment = old_text.slice(old_pos..old_pos + len);
				let old_end_byte = start_byte + segment.len_bytes() as u32;
				let old_end_point = add_delta_rope(start_point, segment);

				edits.push(InputEdit {
					start_byte,
					old_end_byte,
					new_end_byte: start_byte,
					start_point,
					old_end_point,
					new_end_point: start_point,
				});
				old_pos += len;
			}
			Operation::Insert(s) => {
				let start_byte = current_byte;
				let start_point = current_point;

				let insert_len = s.byte_len() as u32;
				let new_end_point = add_delta(start_point, s.text());

				// Check for subsequent delete (replacement)
				if let Some(Operation::Delete(del_len)) = iter.peek() {
					let del_segment = old_text.slice(old_pos..old_pos + del_len);
					let old_end_byte = start_byte + del_segment.len_bytes() as u32;
					let old_end_point = add_delta_rope(start_point, del_segment);
					iter.next();
					old_pos += del_len;

					edits.push(InputEdit {
						start_byte,
						old_end_byte,
						new_end_byte: start_byte + insert_len,
						start_point,
						old_end_point,
						new_end_point,
					});
				} else {
					edits.push(InputEdit {
						start_byte,
						old_end_byte: start_byte,
						new_end_byte: start_byte + insert_len,
						start_point,
						old_end_point: start_point,
						new_end_point,
					});
				}
				current_byte += insert_len;
				current_point = new_end_point;
			}
		}
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

/// Recursive implementation of tree pretty-printing.
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
	use xeno_primitives::{Rope, Transaction};

	use super::*;

	#[test]
	fn test_generate_edits_insert() {
		use xeno_primitives::transaction::Change;
		let doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 5,
			end: 5,
			replacement: Some(" beautiful".into()),
		}];
		let tx = Transaction::change(doc.slice(..), changes);

		let edits = generate_edits(doc.slice(..), tx.changes());
		assert_eq!(edits.len(), 1);
		assert_eq!(edits[0].start_byte, 5);
		assert_eq!(edits[0].old_end_byte, 5);
		assert_eq!(edits[0].new_end_byte, 15);
	}

	#[test]
	fn test_generate_edits_delete() {
		use xeno_primitives::transaction::Change;
		let doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 5,
			end: 11,
			replacement: None,
		}];
		let tx = Transaction::change(doc.slice(..), changes);

		let edits = generate_edits(doc.slice(..), tx.changes());
		assert_eq!(edits.len(), 1);
		assert_eq!(edits[0].start_byte, 5);
		assert_eq!(edits[0].old_end_byte, 11);
		assert_eq!(edits[0].new_end_byte, 5);
	}

	#[test]
	fn test_generate_edits_replace() {
		use xeno_primitives::transaction::Change;
		let doc = Rope::from("hello world");
		let changes = vec![Change {
			start: 6,
			end: 11,
			replacement: Some("rust".into()),
		}];
		let tx = Transaction::change(doc.slice(..), changes);

		let edits = generate_edits(doc.slice(..), tx.changes());
		assert_eq!(edits.len(), 1);
		assert_eq!(edits[0].start_byte, 6);
		assert_eq!(edits[0].old_end_byte, 11);
		assert_eq!(edits[0].new_end_byte, 10);
	}

	#[test]
	fn test_generate_edits_multi_insert_requires_coordinate_shift() {
		use xeno_primitives::transaction::Change;

		// ASCII => bytes == chars for simple assertions.
		let doc = Rope::from("hello world"); // len_bytes = 11

		// Two inserts in one ChangeSet: at start and at end (in original coordinates).
		let changes = vec![
			Change {
				start: 0,
				end: 0,
				replacement: Some("X".into()),
			},
			Change {
				start: 11,
				end: 11,
				replacement: Some("Y".into()),
			},
		];

		let tx = Transaction::change(doc.slice(..), changes);
		let edits = generate_edits(doc.slice(..), tx.changes());

		assert_eq!(edits.len(), 2);

		// First insert at 0 is fine.
		assert_eq!(edits[0].start_byte, 0);
		assert_eq!(edits[0].old_end_byte, 0);
		assert_eq!(edits[0].new_end_byte, 1);

		// If InputEdits are applied sequentially (Tree::edit style),
		// the second insert’s coordinates must be shifted by +1 byte due to the prior insert.
		assert_eq!(edits[1].start_byte, 12, "start_byte should be shifted");
		assert_eq!(edits[1].old_end_byte, 12, "old_end_byte should be shifted");
		assert_eq!(edits[1].new_end_byte, 13, "new_end_byte should be shifted");

		// Bonus: Point.col is also in bytes; should match shifted coordinates on row 0.
		assert_eq!(edits[1].start_point.row, 0);
		assert_eq!(edits[1].start_point.col, 12);
		assert_eq!(edits[1].new_end_point.col, 13);
	}
}
