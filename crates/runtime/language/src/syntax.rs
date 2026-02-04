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

	fn point_at_char(text: RopeSlice, char_idx: usize) -> Point {
		let row = text.char_to_line(char_idx) as u32;
		let line_start_char = text.line_to_char(row as usize);
		let in_line_chars = char_idx.saturating_sub(line_start_char);
		// tree-sitter Point.column is in BYTES, not chars
		let line = text.line(row as usize);
		let col_bytes = line.char_to_byte(in_line_chars) as u32;
		Point {
			row,
			col: col_bytes,
		}
	}

	fn point_after_insert(start: Point, inserted: &str) -> Point {
		if inserted.is_empty() {
			return start;
		}
		let bytes = inserted.as_bytes();
		let mut rows = 0u32;
		let mut last_line_bytes = 0u32;
		let mut cur = 0u32;
		for &b in bytes {
			cur += 1;
			if b == b'\n' {
				rows += 1;
				last_line_bytes = 0;
				cur = 0;
			} else {
				last_line_bytes = cur;
			}
		}
		if rows == 0 {
			Point {
				row: start.row,
				col: start.col + last_line_bytes,
			}
		} else {
			Point {
				row: start.row + rows,
				col: last_line_bytes,
			}
		}
	}

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
				let start_point = point_at_char(old_text, old_pos);
				let old_end_point = point_at_char(old_text, old_end);

				edits.push(InputEdit {
					start_byte,
					old_end_byte,
					new_end_byte: start_byte,
					start_point,
					old_end_point,
					new_end_point: start_point,
				});
			}
			Operation::Insert(s) => {
				let start_byte = old_text.char_to_byte(old_pos) as u32;
				let start_point = point_at_char(old_text, old_pos);
				let new_end_point = point_after_insert(start_point, &s.text);

				// Check for subsequent delete (replacement)
				if let Some(Operation::Delete(del_len)) = iter.peek() {
					old_end = old_pos + del_len;
					let old_end_byte = old_text.char_to_byte(old_end) as u32;
					let old_end_point = point_at_char(old_text, old_end);
					iter.next();

					edits.push(InputEdit {
						start_byte,
						old_end_byte,
						new_end_byte: start_byte + s.text.len() as u32,
						start_point,
						old_end_point,
						new_end_point,
					});
				} else {
					edits.push(InputEdit {
						start_byte,
						old_end_byte: start_byte,
						new_end_byte: start_byte + s.text.len() as u32,
						start_point,
						old_end_point: start_point,
						new_end_point,
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
}
