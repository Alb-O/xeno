use xeno_primitives::transaction::Bias;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::highlight::{HighlightSpan, HighlightStyles};
use xeno_runtime_language::syntax::Syntax;
use xeno_tui::style::Style;

use crate::syntax_manager::HighlightProjectionCtx;

#[inline]
pub(super) fn line_to_byte_or_eof(rope: &Rope, line: usize) -> u32 {
	if line < rope.len_lines() {
		rope.line_to_byte(line) as u32
	} else {
		rope.len_bytes() as u32
	}
}

pub(super) fn remap_stale_span_to_current(
	span: &HighlightSpan,
	old_rope: &Rope,
	new_rope: &Rope,
	changes: &ChangeSet,
) -> Option<(u32, u32)> {
	let old_len_bytes = old_rope.len_bytes();
	let old_start_byte = (span.start as usize).min(old_len_bytes);
	let old_end_byte = (span.end as usize).min(old_len_bytes);

	let old_start_char = old_rope.byte_to_char(old_start_byte);
	let old_end_char = old_rope.byte_to_char(old_end_byte);
	let new_len_chars = new_rope.len_chars();

	// Preserve half-open interval semantics when mapping through edits.
	let new_start_char = changes
		.map_pos(old_start_char, Bias::Right)
		.min(new_len_chars);
	let new_end_char = changes.map_pos(old_end_char, Bias::Left).min(new_len_chars);
	if new_start_char >= new_end_char {
		return None;
	}

	let new_start_byte = new_rope.char_to_byte(new_start_char) as u32;
	let new_end_byte = new_rope.char_to_byte(new_end_char) as u32;
	(new_start_byte < new_end_byte).then_some((new_start_byte, new_end_byte))
}

pub(super) fn build_tile_spans<F>(
	rope: &Rope,
	syntax: &Syntax,
	language_loader: &LanguageLoader,
	style_resolver: &F,
	start_line: usize,
	end_line: usize,
) -> Vec<(HighlightSpan, Style)>
where
	F: Fn(&str) -> Style,
{
	// Hard rule: if the tree is out of bounds for the rope, return empty.
	// This protects against crashes in tree-sitter highlighter.
	let rope_len_bytes = rope.len_bytes() as u32;
	if syntax.tree().root_node().end_byte() > rope_len_bytes {
		return Vec::new();
	}

	let tile_start_byte = line_to_byte_or_eof(rope, start_line);
	let tile_end_byte = if end_line < rope.len_lines() {
		rope.line_to_byte(end_line) as u32
	} else {
		rope_len_bytes
	};

	let highlight_styles = HighlightStyles::new(
		xeno_registry::themes::SyntaxStyles::scope_names(),
		style_resolver,
	);

	let highlighter = syntax.highlighter(
		rope.slice(..),
		language_loader,
		tile_start_byte..tile_end_byte,
	);

	highlighter
		.filter_map(|mut span| {
			// Clamp spans to both rope bounds and tile bounds to ensure safety and determinism
			span.start = span.start.max(tile_start_byte).min(tile_end_byte);
			span.end = span.end.max(tile_start_byte).min(tile_end_byte);

			if span.start >= span.end {
				return None;
			}

			let style = highlight_styles.style_for_highlight(span.highlight);
			Some((span, style))
		})
		.collect()
}

pub(super) fn project_spans_to_target(
	source_spans: &[(HighlightSpan, Style)],
	projection: HighlightProjectionCtx<'_>,
	target_rope: &Rope,
) -> Vec<(HighlightSpan, Style)> {
	source_spans
		.iter()
		.filter_map(|(span, style)| {
			let (start, end) = remap_stale_span_to_current(
				span,
				projection.base_rope,
				target_rope,
				projection.composed_changes,
			)?;
			Some((
				HighlightSpan {
					start,
					end,
					highlight: span.highlight,
				},
				*style,
			))
		})
		.collect()
}
