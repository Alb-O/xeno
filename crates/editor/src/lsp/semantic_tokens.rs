//! Semantic token cache, LSP decode, and style mapping.
//!
//! Manages per-buffer semantic token caches with the same generation-gated
//! pattern as inlay hints. Decodes the LSP relative-encoded 5-tuple format
//! into byte-range highlight spans with resolved styles.

use std::collections::HashMap;
use std::sync::Arc;

use xeno_language::HighlightSpan;
use xeno_lsp::{OffsetEncoding, lsp_types};
use xeno_primitives::{Rope, Style};

use crate::buffer::ViewId;

/// Decoded semantic token spans with pre-resolved styles, ready for merging
/// into the highlight index.
pub(crate) type SemanticTokenSpans = Vec<(HighlightSpan, Style)>;

/// Per-buffer semantic token cache entry.
struct CacheEntry {
	/// Document revision when tokens were fetched.
	doc_rev: u64,
	/// Visible line range (start, end) when tokens were fetched.
	line_range: (usize, usize),
	/// Decoded + styled spans ready for highlight index merge.
	spans: Arc<SemanticTokenSpans>,
	/// Generation counter for in-flight de-duplication.
	request_gen: u64,
}

/// Manages semantic token caches for all open buffers.
///
/// Mirrors the inlay hint cache pattern: generation counters for de-duplication,
/// in-flight tracking to prevent duplicate requests, and line-range validity
/// checking for cache hits.
pub(crate) struct SemanticTokenCache {
	entries: HashMap<ViewId, CacheEntry>,
	/// Monotonic generation counters per buffer (survives cache invalidation).
	gens: HashMap<ViewId, u64>,
	/// Generation of the currently in-flight request per buffer.
	in_flight: HashMap<ViewId, u64>,
	/// Global epoch — incremented on `invalidate_all()` to reject stale responses.
	epoch: u64,
}

impl SemanticTokenCache {
	pub fn new() -> Self {
		Self {
			entries: HashMap::new(),
			gens: HashMap::new(),
			in_flight: HashMap::new(),
			epoch: 0,
		}
	}

	/// Returns the current global epoch.
	pub fn epoch(&self) -> u64 {
		self.epoch
	}

	/// Returns cached spans for a buffer, if valid for the given doc revision and line range.
	pub fn get(&self, buffer_id: ViewId, doc_rev: u64, line_lo: usize, line_hi: usize) -> Option<&Arc<SemanticTokenSpans>> {
		let entry = self.entries.get(&buffer_id)?;
		if entry.doc_rev == doc_rev && entry.line_range.0 <= line_lo && entry.line_range.1 >= line_hi {
			Some(&entry.spans)
		} else {
			None
		}
	}

	/// Stores resolved spans for a buffer and clears the in-flight marker.
	///
	/// If `epoch` doesn't match the current cache epoch, the result is from
	/// before a refresh/invalidation and is silently dropped.
	pub fn insert(&mut self, buffer_id: ViewId, doc_rev: u64, line_lo: usize, line_hi: usize, generation: u64, epoch: u64, spans: Arc<SemanticTokenSpans>) {
		self.clear_in_flight(buffer_id, generation);
		if epoch != self.epoch {
			return;
		}
		self.entries.insert(
			buffer_id,
			CacheEntry {
				doc_rev,
				line_range: (line_lo, line_hi),
				spans,
				request_gen: generation,
			},
		);
	}

	/// Invalidates all caches (e.g. on `workspace/semanticTokens/refresh` or theme change).
	///
	/// Bumps the global epoch so in-flight responses from before the
	/// invalidation are silently dropped on arrival.
	pub fn invalidate_all(&mut self) {
		self.epoch += 1;
		self.entries.clear();
		self.in_flight.clear();
	}

	/// Returns the current generation for a buffer.
	pub fn generation(&self, buffer_id: ViewId) -> u64 {
		self.gens.get(&buffer_id).copied().unwrap_or(0)
	}

	/// Bumps generation for a buffer and returns the new value.
	pub fn bump_generation(&mut self, buffer_id: ViewId) -> u64 {
		let g = self.gens.entry(buffer_id).or_insert(0);
		*g += 1;
		*g
	}

	/// Returns true if a request is already in flight for this buffer.
	pub fn is_in_flight(&self, buffer_id: ViewId) -> bool {
		self.in_flight.contains_key(&buffer_id)
	}

	/// Marks a generation as in-flight for a buffer.
	pub fn mark_in_flight(&mut self, buffer_id: ViewId, generation: u64) {
		self.in_flight.insert(buffer_id, generation);
	}

	/// Clears the in-flight marker if it matches the given generation.
	pub fn clear_in_flight(&mut self, buffer_id: ViewId, generation: u64) {
		if self.in_flight.get(&buffer_id) == Some(&generation) {
			self.in_flight.remove(&buffer_id);
		}
	}
}

/// Decodes LSP semantic tokens (relative 5-tuple encoding) into highlight spans.
///
/// The LSP semantic token format is a flat array of `u32` values in groups of 5:
/// `[deltaLine, deltaStartChar, length, tokenType, tokenModifiersBitset]`.
///
/// This function accumulates absolute positions, converts LSP character offsets
/// to byte offsets using encoding-aware helpers, and maps token types to styles
/// via the provided legend and style resolver.
///
/// Tokens that fall outside the document or have unknown types are silently skipped.
pub(crate) fn decode_semantic_tokens(
	tokens: &[lsp_types::SemanticToken],
	rope: &Rope,
	encoding: OffsetEncoding,
	legend: &lsp_types::SemanticTokensLegend,
	style_resolver: impl Fn(&str) -> Option<Style>,
) -> SemanticTokenSpans {
	let mut result = Vec::with_capacity(tokens.len());
	let mut line: u32 = 0;
	let mut start_char: u32 = 0;

	let total_lines = rope.len_lines();

	for token in tokens {
		// Accumulate absolute position from deltas.
		if token.delta_line > 0 {
			line += token.delta_line;
			start_char = token.delta_start;
		} else {
			start_char += token.delta_start;
		}

		let line_idx = line as usize;
		if line_idx >= total_lines {
			continue;
		}

		// Convert LSP character offset to byte offset.
		let lsp_start = lsp_types::Position { line, character: start_char };
		let lsp_end = lsp_types::Position {
			line,
			character: start_char + token.length,
		};

		let Some(start_char_idx) = xeno_lsp::lsp_position_to_char(rope, lsp_start, encoding) else {
			continue;
		};
		// Clamp end to line end when the token length extends past EOL.
		let end_char_idx = xeno_lsp::lsp_position_to_char(rope, lsp_end, encoding).unwrap_or_else(|| {
			let line_start = rope.line_to_char(line_idx);
			let line_len = rope.line(line_idx).len_chars();
			line_start + line_len
		});

		let start_byte = rope.char_to_byte(start_char_idx) as u32;
		let end_byte = rope.char_to_byte(end_char_idx) as u32;

		if start_byte >= end_byte {
			continue;
		}

		// Resolve token type to style.
		let type_idx = token.token_type as usize;
		let Some(type_name) = legend.token_types.get(type_idx) else {
			continue;
		};

		let Some(scope) = semantic_token_type_to_scope(type_name.as_str()) else {
			continue;
		};
		let Some(style) = style_resolver(scope) else {
			continue;
		};

		result.push((
			HighlightSpan {
				start: start_byte,
				end: end_byte,
				highlight: xeno_language::Highlight::new(0),
			},
			style,
		));
	}

	result
}

/// Maps an LSP semantic token type name to a tree-sitter-compatible scope.
///
/// Returns `None` for unknown token types to prevent wash-out of syntax
/// highlighting with a default/plain style. Only recognized types are mapped.
fn semantic_token_type_to_scope(token_type: &str) -> Option<&str> {
	Some(match token_type {
		"namespace" => "module",
		"type" | "class" | "enum" | "interface" | "struct" | "typeParameter" => "type",
		"parameter" => "variable.parameter",
		"variable" => "variable",
		"property" | "enumMember" => "variable.other.member",
		"event" => "variable.other.member",
		"function" => "function",
		"method" => "function.method",
		"macro" => "function.macro",
		"keyword" | "modifier" => "keyword",
		"comment" => "comment",
		"string" => "string",
		"number" => "constant.numeric",
		"regexp" => "string.regexp",
		"operator" => "operator",
		"decorator" => "attribute",
		_ => return None,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Verifies delta decoding accumulates absolute positions correctly.
	#[test]
	fn decode_delta_accumulation() {
		let rope = Rope::from("fn foo() {}\nfn bar() {}\n");
		let legend = lsp_types::SemanticTokensLegend {
			token_types: vec![lsp_types::SemanticTokenType::KEYWORD, lsp_types::SemanticTokenType::FUNCTION],
			token_modifiers: vec![],
		};

		let tokens = vec![
			// Line 0, char 0, len 2 = "fn", type 0 (keyword)
			lsp_types::SemanticToken {
				delta_line: 0,
				delta_start: 0,
				length: 2,
				token_type: 0,
				token_modifiers_bitset: 0,
			},
			// Line 0, char 3, len 3 = "foo", type 1 (function)
			lsp_types::SemanticToken {
				delta_line: 0,
				delta_start: 3,
				length: 3,
				token_type: 1,
				token_modifiers_bitset: 0,
			},
			// Line 1, char 0, len 2 = "fn", type 0 (keyword)
			lsp_types::SemanticToken {
				delta_line: 1,
				delta_start: 0,
				length: 2,
				token_type: 0,
				token_modifiers_bitset: 0,
			},
			// Line 1, char 3, len 3 = "bar", type 1 (function)
			lsp_types::SemanticToken {
				delta_line: 0,
				delta_start: 3,
				length: 3,
				token_type: 1,
				token_modifiers_bitset: 0,
			},
		];

		let kw_style = Style {
			fg: Some(xeno_primitives::Color::Rgb(255, 0, 0)),
			..Default::default()
		};
		let fn_style = Style {
			fg: Some(xeno_primitives::Color::Rgb(0, 255, 0)),
			..Default::default()
		};

		let spans = decode_semantic_tokens(&tokens, &rope, OffsetEncoding::Utf8, &legend, |scope| match scope {
			"keyword" => Some(kw_style),
			"function" => Some(fn_style),
			_ => None,
		});

		assert_eq!(spans.len(), 4);

		// "fn" on line 0: bytes 0..2
		assert_eq!(spans[0].0.start, 0);
		assert_eq!(spans[0].0.end, 2);
		assert_eq!(spans[0].1, kw_style);

		// "foo" on line 0: bytes 3..6
		assert_eq!(spans[1].0.start, 3);
		assert_eq!(spans[1].0.end, 6);
		assert_eq!(spans[1].1, fn_style);

		// "fn" on line 1: bytes 12..14
		assert_eq!(spans[2].0.start, 12);
		assert_eq!(spans[2].0.end, 14);
		assert_eq!(spans[2].1, kw_style);

		// "bar" on line 1: bytes 15..18
		assert_eq!(spans[3].0.start, 15);
		assert_eq!(spans[3].0.end, 18);
		assert_eq!(spans[3].1, fn_style);
	}

	/// Verifies that out-of-bounds tokens are skipped.
	#[test]
	fn decode_out_of_bounds_skipped() {
		let rope = Rope::from("hi\n");
		let legend = lsp_types::SemanticTokensLegend {
			token_types: vec![lsp_types::SemanticTokenType::KEYWORD],
			token_modifiers: vec![],
		};

		let tokens = vec![lsp_types::SemanticToken {
			delta_line: 5, // Line 5 doesn't exist
			delta_start: 0,
			length: 2,
			token_type: 0,
			token_modifiers_bitset: 0,
		}];

		let spans = decode_semantic_tokens(&tokens, &rope, OffsetEncoding::Utf8, &legend, |_| Some(Style::default()));

		assert!(spans.is_empty());
	}

	/// Verifies that unknown token types are skipped.
	#[test]
	fn decode_unknown_token_type_skipped() {
		let rope = Rope::from("hello\n");
		let legend = lsp_types::SemanticTokensLegend {
			token_types: vec![lsp_types::SemanticTokenType::KEYWORD],
			token_modifiers: vec![],
		};

		let tokens = vec![lsp_types::SemanticToken {
			delta_line: 0,
			delta_start: 0,
			length: 5,
			token_type: 99, // Out of legend bounds
			token_modifiers_bitset: 0,
		}];

		let spans = decode_semantic_tokens(&tokens, &rope, OffsetEncoding::Utf8, &legend, |_| Some(Style::default()));

		assert!(spans.is_empty());
	}

	/// Verifies that tokens extending past EOL are clamped to line end.
	#[test]
	fn decode_eol_clamp() {
		let rope = Rope::from("ab\n");
		let legend = lsp_types::SemanticTokensLegend {
			token_types: vec![lsp_types::SemanticTokenType::KEYWORD],
			token_modifiers: vec![],
		};

		// Token at line 0, char 0, length 10 — extends way past the 2-char line.
		let tokens = vec![lsp_types::SemanticToken {
			delta_line: 0,
			delta_start: 0,
			length: 10,
			token_type: 0,
			token_modifiers_bitset: 0,
		}];

		let spans = decode_semantic_tokens(&tokens, &rope, OffsetEncoding::Utf8, &legend, |_| Some(Style::default()));

		assert_eq!(spans.len(), 1);
		// Clamped to "ab" = bytes 0..2 (lsp_position_to_char clamps to line end).
		assert_eq!(spans[0].0.start, 0);
		assert_eq!(spans[0].0.end, 2);
	}
}
