//! Inlay hint types for buffer rendering.
//!
//! Render-side types for displaying inlay hints (type annotations, parameter names, etc.)
//! as virtual text inserted between real document characters. LSP conversion lives in the
//! `lsp::inlay_hints` module to keep LSP dependencies out of the render path.

use std::collections::HashMap;
use std::sync::Arc;

/// A single inlay hint span to render on a line.
#[derive(Debug, Clone)]
pub struct InlayHintSpan {
	/// Character position on this line where the hint is inserted (0-indexed).
	pub pos_char: usize,
	/// Display text for the hint.
	pub text: Arc<str>,
	/// Display width in columns (pre-computed).
	pub cols: u16,
	/// Whether to add a space before the hint text.
	pub pad_left: bool,
	/// Whether to add a space after the hint text.
	pub pad_right: bool,
	/// Hint kind: 1 = Type, 2 = Parameter, 0 = Other.
	pub kind: u8,
}

/// Map from line number (0-indexed) to sorted inlay hint spans on that line.
pub type InlayHintRangeMap = HashMap<usize, Vec<InlayHintSpan>>;

/// View into inlay hints for a single line, providing fast queries.
pub struct InlayHintLine<'a> {
	spans: &'a [InlayHintSpan],
}

impl<'a> InlayHintLine<'a> {
	pub fn new(spans: &'a [InlayHintSpan]) -> Self {
		Self { spans }
	}

	pub fn empty() -> Self {
		Self { spans: &[] }
	}

	pub fn is_empty(&self) -> bool {
		self.spans.is_empty()
	}

	/// Returns all spans inserted at exactly `pos_char`.
	#[allow(dead_code)]
	pub fn at_pos(&self, pos_char: usize) -> impl Iterator<Item = &InlayHintSpan> {
		self.spans.iter().filter(move |s| s.pos_char == pos_char)
	}

	/// Total display columns added by hints with `pos_char` strictly less than `pos_char`.
	///
	/// Cursor at a given position sits *before* any inlay at the same position,
	/// so we use strict `<` here.
	pub fn cols_before(&self, pos_char: usize) -> u16 {
		self.spans
			.iter()
			.filter(|s| s.pos_char < pos_char)
			.map(|s| s.cols + s.pad_left as u16 + s.pad_right as u16)
			.sum()
	}

	/// All spans on this line.
	pub fn spans(&self) -> &[InlayHintSpan] {
		self.spans
	}
}
