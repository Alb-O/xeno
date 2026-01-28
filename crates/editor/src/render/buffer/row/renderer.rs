use std::path::Path;

use xeno_primitives::Rope;
use xeno_registry::gutter::GutterAnnotations;
use xeno_tui::style::Style;
use xeno_tui::text::Line;

use super::super::cell_style::{CellStyleInput, CursorStyleSet, resolve_cell_style};
use super::super::context::types::{BufferRenderContext, CursorStyles, RenderLayout};
use super::super::index::{CursorKind, HighlightIndex, OverlayIndex};
use super::super::plan::LineSlice;
use super::super::style_layers::{LineStyleContext, blend};
use super::shaper::SegmentGlyphIter;
use super::span_builder::SpanRunBuilder;
use crate::render::wrap::WrappedSegment;

pub struct RowRenderInput<'a> {
	pub ctx: &'a BufferRenderContext<'a>,
	pub theme_cursor_styles: &'a CursorStyles,
	pub cursor_style_set: CursorStyleSet,
	pub line_style: LineStyleContext,

	pub layout: &'a RenderLayout,
	pub buffer_path: Option<&'a Path>,
	pub is_focused: bool,
	pub use_block_cursor: bool,
	pub tab_width: usize,
	pub doc_content: &'a Rope,

	pub line: Option<&'a LineSlice>,
	pub segment: Option<&'a WrappedSegment>,
	pub is_continuation: bool,
	pub is_last_segment: bool,

	pub highlight: &'a HighlightIndex,
	pub overlays: &'a OverlayIndex,

	pub line_annotations: GutterAnnotations,
}

pub struct TextRowRenderer;

impl TextRowRenderer {
	pub fn render_row(input: &RowRenderInput<'_>) -> Line<'static> {
		let mut builder = SpanRunBuilder::new();
		let text_width = input.layout.text_width;

		match (input.line, input.segment) {
			(Some(line), Some(segment)) => {
				let shaper = SegmentGlyphIter::new(line, segment, input.tab_width, text_width);
				let mut cols_used = 0;

				for glyph in shaper {
					// Virtual glyphs (indent/tab expansion) don't participate in overlays
					// to avoid cursor/selection duplication on expanded tabs.
					let (syntax_style, in_selection, cursor_kind) = if glyph.is_virtual {
						(None, false, CursorKind::None)
					} else {
						(
							input.highlight.style_at(glyph.doc_byte),
							input
								.overlays
								.in_selection(line.line_idx, glyph.line_char_off),
							input.overlays.cursor_kind(glyph.doc_char, input.is_focused),
						)
					};

					let cell_input = CellStyleInput {
						line_ctx: &input.line_style,
						syntax_style,
						in_selection,
						is_primary_cursor: cursor_kind == CursorKind::Primary,
						is_focused: input.is_focused,
						cursor_styles: &input.cursor_style_set,
						base_style: input.theme_cursor_styles.base,
					};

					let resolved = resolve_cell_style(cell_input);
					let style = if cursor_kind != CursorKind::None
						&& (input.use_block_cursor || !input.is_focused)
					{
						resolved.cursor
					} else {
						input.ctx.apply_diagnostic_underline(
							line.line_idx,
							glyph.line_char_off,
							resolved.non_cursor,
						)
					};

					builder.push_text(style, &glyph.ch.to_string());
					cols_used += glyph.width;
				}

				// Handle EOL cursor
				if cols_used < text_width && input.is_last_segment {
					let line_end = line.end_char_incl_nl;
					let content_end = line.content_end_char;

					// Check if any cursor is at EOL
					let mut eol_cursor_kind = CursorKind::None;
					for pos in content_end..=line_end {
						let kind = input.overlays.cursor_kind(pos, input.is_focused);
						if kind != CursorKind::None {
							eol_cursor_kind = kind;
							break;
						}
					}

					if eol_cursor_kind != CursorKind::None
						&& (input.use_block_cursor || !input.is_focused)
					{
						let cursor_style = match eol_cursor_kind {
							CursorKind::Primary => input.theme_cursor_styles.primary,
							CursorKind::Secondary => input.theme_cursor_styles.secondary,
							CursorKind::Unfocused => input.theme_cursor_styles.unfocused,
							_ => unreachable!(),
						};

						let has_newline = line.has_newline;
						let eol_char = if has_newline { "¬" } else { " " };
						let eol_style = match (cursor_style.fg, cursor_style.bg) {
							(Some(fg), Some(bg)) => cursor_style.fg(fg.blend(bg, 0.35)),
							_ => cursor_style,
						};
						builder.push_text(eol_style, eol_char);
						cols_used += 1;
					}
				}

				// Fill remaining
				if cols_used < text_width {
					let fill_count = text_width - cols_used;
					if let Some(bg) = input.line_style.fill_bg() {
						builder.push_spaces(Style::default().bg(bg), fill_count);
					} else if !input.is_last_segment {
						// Continuation fill
						let dim_color = input
							.ctx
							.theme
							.colors
							.ui
							.gutter_fg
							.blend(input.ctx.theme.colors.ui.bg, blend::GUTTER_DIM_ALPHA);
						builder.push_spaces(Style::default().fg(dim_color), fill_count);
					} else {
						// Normal line fill
						use super::super::fill::FillConfig;
						if let Some(fill_span) =
							FillConfig::from_bg(input.line_style.fill_bg()).fill_span(fill_count)
						{
							builder.push_text(fill_span.style, &fill_span.content);
						}
					}
				}
			}
			_ => {
				// Phantom or NonText fill
				let bg = input.line_style.base_bg;
				builder.push_spaces(Style::default().bg(bg), text_width);
			}
		}

		let mut line = Line::from(builder.finish());
		if let Some(bg) = input.line_style.fill_bg() {
			line = line.style(Style::default().bg(bg));
		}
		line
	}
}

pub struct GutterRenderer;

impl GutterRenderer {
	pub fn render_row(input: &RowRenderInput<'_>) -> Line<'static> {
		let spans = if let Some(line) = input.line {
			input.layout.gutter_layout.render_line(
				line.line_idx,
				input.layout.total_lines,
				&input.line_style,
				input.is_continuation,
				input.doc_content.line(line.line_idx),
				input.buffer_path,
				&input.line_annotations,
				input.ctx.theme,
			)
		} else {
			input
				.layout
				.gutter_layout
				.render_empty_line(input.ctx.theme)
		};

		Line::from(spans)
	}
}
