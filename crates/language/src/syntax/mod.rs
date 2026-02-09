//! Core syntax parsing and incremental updates.
//!
//! This module wraps tree-house's Syntax type to provide incremental parsing
//! that integrates with Xeno's ChangeSet/Transaction system.

use std::ops::RangeBounds;
use std::sync::Arc;
use std::time::Duration;

use ropey::RopeSlice;
use thiserror::Error;
pub use tree_house::SealedSource;
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
	/// Metadata for viewport-first trees.
	pub viewport: Option<ViewportMetadata>,
}

#[derive(Debug, Clone)]
pub struct ViewportMetadata {
	pub base_offset: u32,
	pub real_len: u32,
	pub sealed_source: Arc<SealedSource>,
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
		Ok(Self {
			inner,
			opts,
			viewport: None,
		})
	}

	/// Creates a new viewport-first syntax tree from a window.
	pub fn new_viewport(
		sealed: Arc<SealedSource>,
		language: Language,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
		base_offset: u32,
	) -> Result<Self, SyntaxError> {
		let loader = loader.with_injections(matches!(opts.injections, InjectionPolicy::Eager));
		let inner = tree_house::Syntax::new(sealed.slice(), language, opts.parse_timeout, &loader)?;
		Ok(Self {
			inner,
			opts,
			viewport: Some(ViewportMetadata {
				base_offset,
				real_len: sealed.real_len_bytes,
				sealed_source: sealed,
			}),
		})
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
		// Viewport trees become full trees after an update (or at least lose their viewport status)
		// but in Xeno we typically replace them with a background full parse result.
		self.viewport = None;
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

	/// Returns true if this is a viewport-first tree.
	pub fn is_partial(&self) -> bool {
		self.viewport.is_some()
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
		if let Some(meta) = &self.viewport {
			Highlighter::new_mapped(
				&self.inner,
				meta.sealed_source.slice(),
				loader,
				range,
				meta.base_offset,
				meta.base_offset + meta.real_len,
			)
		} else {
			Highlighter::new(&self.inner, source, loader, range)
		}
	}
}

#[derive(Debug, Clone)]
pub struct ViewportRepair {
	pub enabled: bool,
	pub max_scan_bytes: u32,
	pub prefer_real_closer: bool,
	pub max_forward_search_bytes: u32,
	pub rules: Vec<ViewportRepairRule>,
}

#[derive(Debug, Clone)]
pub enum ViewportRepairRule {
	BlockComment {
		open: String,
		close: String,
		nestable: bool,
	},
	String {
		quote: String,
		escape: Option<String>,
	},
	LineComment {
		start: String,
	},
}

impl ViewportRepair {
	/// Scans the window to determine the synthetic suffix needed to close multi-line constructs.
	///
	/// Optionally performs a forward search in the full document to find a real closer.
	pub fn scan(&self, window: RopeSlice<'_>, forward_haystack: Option<RopeSlice<'_>>) -> String {
		if !self.enabled || window.len_bytes() == 0 {
			return String::new();
		}

		// MVP byte-oriented scanner
		let mut block_comment_depth = 0;
		let mut in_string: Option<usize> = None; // index into self.rules
		let mut in_line_comment = false;

		let bytes: Vec<u8> = window.bytes().take(self.max_scan_bytes as usize).collect();

		let mut i = 0;
		while i < bytes.len() {
			if in_line_comment {
				if bytes[i] == b'\n' {
					in_line_comment = false;
				}
				i += 1;
				continue;
			}

			if let Some(rule_idx) = in_string {
				let rule = &self.rules[rule_idx];
				if let ViewportRepairRule::String { quote, escape } = rule {
					if let Some(esc) = escape
						&& bytes[i..].starts_with(esc.as_bytes())
					{
						i += esc.len();
						i += 1; // skip escaped char
						continue;
					}
					if bytes[i..].starts_with(quote.as_bytes()) {
						in_string = None;
						i += quote.len();
						continue;
					}
				}
				i += 1;
				continue;
			}

			// Not in line comment or string
			let mut matched = false;
			for (idx, rule) in self.rules.iter().enumerate() {
				match rule {
					ViewportRepairRule::LineComment { start } => {
						if bytes[i..].starts_with(start.as_bytes()) {
							in_line_comment = true;
							i += start.len();
							matched = true;
							break;
						}
					}
					ViewportRepairRule::String { quote, .. } => {
						if bytes[i..].starts_with(quote.as_bytes()) {
							in_string = Some(idx);
							i += quote.len();
							matched = true;
							break;
						}
					}
					ViewportRepairRule::BlockComment {
						open,
						close,
						nestable,
					} => {
						if bytes[i..].starts_with(open.as_bytes()) {
							block_comment_depth += 1;
							i += open.len();
							matched = true;
							if !*nestable {
								// skip until closer
								while i < bytes.len() {
									if bytes[i..].starts_with(close.as_bytes()) {
										block_comment_depth -= 1;
										i += close.len();
										break;
									}
									i += 1;
								}
							}
							break;
						} else if bytes[i..].starts_with(close.as_bytes()) {
							if block_comment_depth > 0 {
								block_comment_depth -= 1;
							}
							i += close.len();
							matched = true;
							break;
						}
					}
				}
			}

			if !matched {
				i += 1;
			}
		}

		if block_comment_depth == 0 && in_string.is_none() {
			return String::new();
		}

		// Check for real closer forward if requested
		if self.prefer_real_closer
			&& let Some(haystack) = forward_haystack
		{
			let search_limit = self.max_forward_search_bytes as usize;
			let search_bytes: Vec<u8> = haystack.bytes().take(search_limit).collect();

			if block_comment_depth > 0 {
				if let Some(ViewportRepairRule::BlockComment { close, .. }) = self
					.rules
					.iter()
					.find(|r| matches!(r, ViewportRepairRule::BlockComment { .. }))
					&& search_bytes
						.windows(close.len())
						.any(|w| w == close.as_bytes())
				{
					// Found real closer shortly after window; no synthetic suffix needed
					// (Tree-sitter will find it if we extend the range, but for now we just
					// skip synthesis and rely on the fact that ERROR recovery is localized
					// if the closer exists "soon enough" - actually, better to return the closer
					// as suffix to be safe, or extend the window.)
					//
					// For now, let's just return empty to signal "don't seal, it's fine".
					return String::new();
				}
			} else if let Some(rule_idx) = in_string
				&& let ViewportRepairRule::String { quote, .. } = &self.rules[rule_idx]
				&& search_bytes
					.windows(quote.len())
					.any(|w| w == quote.as_bytes())
			{
				return String::new();
			}
		}

		let mut suffix = String::new();
		if block_comment_depth > 0 {
			// find first block comment rule to get closer
			if let Some(ViewportRepairRule::BlockComment { close, .. }) = self
				.rules
				.iter()
				.find(|r| matches!(r, ViewportRepairRule::BlockComment { .. }))
			{
				for _ in 0..block_comment_depth {
					suffix.push_str(close);
				}
			}
		} else if let Some(rule_idx) = in_string
			&& let ViewportRepairRule::String { quote, .. } = &self.rules[rule_idx]
		{
			suffix.push_str(quote);
		}

		suffix
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
mod tests;
