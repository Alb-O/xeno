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

mod edit_generation;
mod pretty_print;
mod viewport_repair;

use edit_generation::generate_edits;
pub use pretty_print::pretty_print_tree;
pub use viewport_repair::{SealPlan, ViewportRepair, ViewportRepairRule};

/// Default parse timeout (500ms).
const DEFAULT_PARSE_TIMEOUT: Duration = Duration::from_millis(500);

/// Injection handling policy.
///
/// NOTE: actual injection enable/disable is implemented by `LanguageLoader`
/// (tree-house queries + injected language resolution). `SyntaxOptions` just
/// plumbs the intent through the call chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[cfg(test)]
mod tests;
