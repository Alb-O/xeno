//! Buffer rendering for split views.
//!
//! This module provides buffer-agnostic rendering that can render any buffer
//! given a `BufferRenderContext`. This enables proper split view rendering
//! where multiple buffers are rendered simultaneously.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use tome_tui::layout::Rect;
use tome_tui::style::{Modifier, Style};
use tome_tui::text::{Line, Span};
use tome_tui::widgets::Paragraph;
use tome_base::range::CharIdx;
use tome_language::LanguageLoader;
use tome_language::highlight::{HighlightSpan, HighlightStyles};
use tome_manifest::Mode;
use tome_manifest::syntax::SyntaxStyles;
use tome_theme::{Theme, ThemeVariant};

use super::types::{RenderResult, WrapSegment, wrap_line};
use crate::buffer::Buffer;
use crate::editor::extensions::StyleOverlays;

/// Context for rendering a buffer.
///
/// Contains all shared resources needed to render any buffer.
/// This allows the same rendering logic to be applied to any buffer
/// in the editor, enabling proper split view support.
pub struct BufferRenderContext<'a> {
	/// The current theme.
	pub theme: &'static Theme,
	/// Language loader for syntax highlighting.
	pub language_loader: &'a LanguageLoader,
	/// Style overlays (e.g., zen mode dimming).
	pub style_overlays: &'a StyleOverlays,
}

/// Cursor styling configuration for rendering.
pub struct CursorStyles {
	/// Style for the primary (main) cursor.
	pub primary: Style,
	/// Style for secondary (additional) cursors in multi-cursor mode.
	pub secondary: Style,
	/// Base text style.
	pub base: Style,
	/// Selection highlight style.
	pub selection: Style,
}

impl<'a> BufferRenderContext<'a> {
	/// Creates cursor styling configuration based on theme and mode.
	pub fn make_cursor_styles(&self) -> CursorStyles {
		let primary_cursor_style = Style::default()
			.bg(self.theme.colors.ui.cursor_bg.into())
			.fg(self.theme.colors.ui.cursor_fg.into())
			.add_modifier(Modifier::BOLD);

		let secondary_cursor_style = {
			let bg = self.theme.colors.ui.cursor_bg.blend(self.theme.colors.ui.bg, 0.4);
			let fg = self.theme.colors.ui.cursor_fg.blend(self.theme.colors.ui.fg, 0.4);
			Style::default()
				.bg(bg.into())
				.fg(fg.into())
				.add_modifier(Modifier::BOLD)
		};

		let base_style =
			Style::default()
				.fg(self.theme.colors.ui.fg.into())
				.bg(self.theme.colors.ui.bg.into());

		let selection_style = Style::default()
			.bg(self.theme.colors.ui.selection_bg.into())
			.fg(self.theme.colors.ui.selection_fg.into());

		CursorStyles {
			primary: primary_cursor_style,
			secondary: secondary_cursor_style,
			base: base_style,
			selection: selection_style,
		}
	}

	/// Checks if the cursor should be visible (blinking state).
	///
	/// In insert mode, cursors blink on a 200ms cycle. In other modes, cursors
	/// are always visible.
	pub fn cursor_blink_visible(&self, mode: Mode) -> bool {
		let insert_mode = matches!(mode, Mode::Insert);
		if !insert_mode {
			return true;
		}

		let now_ms = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_millis();

		(now_ms / 200).is_multiple_of(2)
	}

	/// Collects syntax highlight spans for a buffer's visible viewport.
	pub fn collect_highlight_spans(
		&self,
		buffer: &Buffer,
		area: Rect,
	) -> Vec<(HighlightSpan, Style)> {
		let Some(ref syntax) = buffer.syntax else {
			return Vec::new();
		};

		let start_line = buffer.scroll_line;
		let end_line = (start_line + area.height as usize).min(buffer.doc.len_lines());

		let start_byte = buffer.doc.line_to_byte(start_line) as u32;
		let end_byte = if end_line < buffer.doc.len_lines() {
			buffer.doc.line_to_byte(end_line) as u32
		} else {
			buffer.doc.len_bytes() as u32
		};

		let highlight_styles = HighlightStyles::new(SyntaxStyles::scope_names(), |scope| {
			self.theme.colors.syntax.resolve(scope)
		});

		let highlighter = syntax.highlighter(
			buffer.doc.slice(..),
			self.language_loader,
			start_byte..end_byte,
		);

		highlighter
			.map(|span| {
				let abstract_style = highlight_styles.style_for_highlight(span.highlight);
				let tome_tui_style: Style = abstract_style.into();
				(span, tome_tui_style)
			})
			.collect()
	}

	/// Looks up the style for a byte position from pre-computed highlight spans.
	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(HighlightSpan, Style)],
	) -> Option<Style> {
		for (span, style) in spans.iter().rev() {
			if byte_pos >= span.start as usize && byte_pos < span.end as usize {
				return Some(*style);
			}
		}
		None
	}

	/// Applies style overlay modifications (e.g., zen mode dimming).
	pub fn apply_style_overlay(&self, byte_pos: usize, style: Option<Style>) -> Option<Style> {
		use tome_tui::animation::Animatable;

		use crate::editor::extensions::StyleMod;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to tome_tui color for blending
				let bg: tome_tui::style::Color = self.theme.colors.ui.bg.into();
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(tome_tui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => style.fg(color),
			StyleMod::Bg(color) => style.bg(color),
		};

		Some(modified)
	}

	/// Renders a buffer into a paragraph widget.
	///
	/// This is the main buffer rendering function that handles:
	/// - Line wrapping and viewport positioning
	/// - Cursor rendering (primary and secondary)
	/// - Selection highlighting
	/// - Gutter with line numbers
	/// - Cursor blinking in insert mode
	///
	/// # Parameters
	/// - `buffer`: The buffer to render
	/// - `area`: The rectangular area to render into
	/// - `use_block_cursor`: Whether to render block-style cursors
	pub fn render_buffer(
		&self,
		buffer: &Buffer,
		area: Rect,
		use_block_cursor: bool,
	) -> RenderResult {
		let total_lines = buffer.doc.len_lines();
		let gutter_width = buffer.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;
		let tab_width = 4usize;

		let cursor = buffer.cursor;
		let ranges = buffer.selection.ranges();
		let primary_cursor = cursor;
		let cursor_heads: HashSet<CharIdx> =
			buffer.selection.ranges().iter().map(|r| r.head).collect();
		let blink_on = self.cursor_blink_visible(buffer.mode());
		let styles = self.make_cursor_styles();

		// Collect syntax highlight spans for the visible viewport
		let highlight_spans = self.collect_highlight_spans(buffer, area);

		let mut output_lines: Vec<Line> = Vec::new();
		let mut current_line_idx = buffer.scroll_line;
		let mut start_segment = buffer.scroll_segment;
		let viewport_height = area.height as usize;

		while output_lines.len() < viewport_height && current_line_idx < total_lines {
			let line_start: CharIdx = buffer.doc.line_to_char(current_line_idx);
			let line_end: CharIdx = if current_line_idx + 1 < total_lines {
				buffer.doc.line_to_char(current_line_idx + 1)
			} else {
				buffer.doc.len_chars()
			};

			let line_text: String = buffer.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let line_content_end: CharIdx = line_start + line_text.chars().count();

			let wrapped_segments = wrap_line(line_text, text_width);
			let num_segments = wrapped_segments.len().max(1);

			for (seg_idx, segment) in wrapped_segments.iter().enumerate().skip(start_segment) {
				if output_lines.len() >= viewport_height {
					break;
				}

				let is_first_segment = seg_idx == 0;
				let is_last_segment = seg_idx == num_segments - 1;

				let line_num_str = if is_first_segment {
					format!(
						"{:>width$} ",
						current_line_idx + 1,
						width = gutter_width as usize - 1
					)
				} else {
					format!("{:>width$} ", "\u{2506}", width = gutter_width as usize - 1)
				};
				let gutter_style = if is_first_segment {
					Style::default().fg(self.theme.colors.ui.gutter_fg.into())
				} else {
					let dim_color =
						self.theme.colors.ui.gutter_fg.blend(self.theme.colors.ui.bg, 0.5);
					Style::default().fg(dim_color.into())
				};

				let mut spans = vec![Span::styled(line_num_str, gutter_style)];

				let seg_char_offset = segment.start_offset;
				let mut seg_col = 0usize;
				for (i, ch) in segment.text.chars().enumerate() {
					if seg_col >= text_width {
						break;
					}

					let doc_pos: CharIdx = line_start + seg_char_offset + i;

					let is_cursor = cursor_heads.contains(&doc_pos);
					let is_primary_cursor = doc_pos == primary_cursor;
					let in_selection = ranges
						.iter()
						.any(|r: &tome_base::range::Range| doc_pos >= r.min() && doc_pos < r.max());

					let cursor_style = if is_primary_cursor {
						styles.primary
					} else {
						styles.secondary
					};
					// Convert char position to byte position for highlight lookup
					let byte_pos = buffer.doc.char_to_byte(doc_pos);
					let syntax_style = self.style_for_byte_pos(byte_pos, &highlight_spans);

					// Apply style overlays (e.g., zen mode dimming)
					let syntax_style = self.apply_style_overlay(byte_pos, syntax_style);

					let non_cursor_style = if in_selection {
						// Invert: syntax fg becomes bg, use contrasting color as fg
						let base = syntax_style.unwrap_or(styles.base);
						let syntax_fg = base.fg.unwrap_or(self.theme.colors.ui.fg.into());
						let text_fg = match self.theme.variant {
							ThemeVariant::Dark => self.theme.colors.ui.bg,
							ThemeVariant::Light => self.theme.colors.ui.fg,
						};
						Style::default()
							.bg(syntax_fg)
							.fg(text_fg.into())
							.add_modifier(base.add_modifier)
					} else {
						syntax_style.unwrap_or(styles.base)
					};
					let style = if is_cursor && use_block_cursor {
						if blink_on { cursor_style } else { styles.base }
					} else {
						non_cursor_style
					};

					if ch == '\t' {
						let remaining = text_width.saturating_sub(seg_col);
						if remaining == 0 {
							break;
						}
						let mut tab_cells = tab_width.saturating_sub(seg_col % tab_width);
						if tab_cells == 0 {
							tab_cells = 1;
						}
						tab_cells = tab_cells.min(remaining);

						if is_cursor && use_block_cursor && blink_on {
							spans.push(Span::styled(" ", cursor_style));
							if tab_cells > 1 {
								spans.push(Span::styled(
									" ".repeat(tab_cells - 1),
									non_cursor_style,
								));
							}
						} else {
							spans.push(Span::styled(" ".repeat(tab_cells), style));
						}

						seg_col += tab_cells;
					} else {
						spans.push(Span::styled(ch.to_string(), style));
						seg_col += 1;
					}
				}

				if !is_last_segment && seg_col < text_width {
					let fill_count = text_width - seg_col;
					let dim_color =
						self.theme.colors.ui.gutter_fg.blend(self.theme.colors.ui.bg, 0.5);
					spans.push(Span::styled(
						" ".repeat(fill_count),
						Style::default().fg(dim_color.into()),
					));
				}

				if is_last_segment {
					let is_last_doc_line = current_line_idx + 1 >= total_lines;
					let cursor_at_eol = cursor_heads.iter().any(|pos: &CharIdx| {
						if is_last_doc_line {
							*pos >= line_content_end && *pos <= line_end
						} else {
							*pos >= line_content_end && *pos < line_end
						}
					});

					if cursor_at_eol {
						let primary_here = if is_last_doc_line {
							primary_cursor >= line_content_end && primary_cursor <= line_end
						} else {
							primary_cursor >= line_content_end && primary_cursor < line_end
						};

						if use_block_cursor && blink_on {
							let cursor_style = if primary_here {
								styles.primary
							} else {
								styles.secondary
							};
							spans.push(Span::styled(" ", cursor_style));
						}
					}
				}

				output_lines.push(Line::from(spans));
			}

			if wrapped_segments.is_empty()
				&& start_segment == 0
				&& output_lines.len() < viewport_height
			{
				let line_num_str = format!(
					"{:>width$} ",
					current_line_idx + 1,
					width = gutter_width as usize - 1
				);
				let gutter_style = Style::default().fg(self.theme.colors.ui.gutter_fg.into());
				let mut spans = vec![Span::styled(line_num_str, gutter_style)];

				let is_last_doc_line = current_line_idx + 1 >= total_lines;
				let cursor_at_eol = cursor_heads.iter().any(|pos: &CharIdx| {
					if is_last_doc_line {
						*pos >= line_start && *pos <= line_end
					} else {
						*pos >= line_start && *pos < line_end
					}
				});
				if cursor_at_eol {
					let primary_here = if is_last_doc_line {
						primary_cursor >= line_start && primary_cursor <= line_end
					} else {
						primary_cursor >= line_start && primary_cursor < line_end
					};

					if use_block_cursor && blink_on {
						let cursor_style = if primary_here {
							styles.primary
						} else {
							styles.secondary
						};
						spans.push(Span::styled(" ", cursor_style));
					}
				}

				output_lines.push(Line::from(spans));
			}

			start_segment = 0;
			current_line_idx += 1;
		}

		while output_lines.len() < viewport_height {
			let line_num_str = format!("{:>width$} ", "~", width = gutter_width as usize - 1);
			let dim_color = self.theme.colors.ui.gutter_fg.blend(self.theme.colors.ui.bg, 0.5);
			output_lines.push(Line::from(vec![Span::styled(
				line_num_str,
				Style::default().fg(dim_color.into()),
			)]));
		}

		RenderResult {
			widget: Paragraph::new(output_lines),
		}
	}
}

/// Ensures the cursor is visible in the buffer's viewport.
///
/// This function adjusts `buffer.scroll_line` and `buffer.scroll_segment` to ensure
/// the primary cursor is visible within the given area. It also updates
/// `buffer.text_width` to match the current rendering context.
pub fn ensure_buffer_cursor_visible(buffer: &mut Buffer, area: Rect) {
	let total_lines = buffer.doc.len_lines();
	let gutter_width = buffer.gutter_width();
	let text_width = area.width.saturating_sub(gutter_width) as usize;
	let viewport_height = area.height as usize;

	buffer.text_width = text_width;

	if buffer.scroll_line >= total_lines {
		buffer.scroll_line = total_lines.saturating_sub(1);
		buffer.scroll_segment = 0;
	}
	buffer.scroll_segment = clamp_segment_for_line(
		buffer,
		buffer.scroll_line,
		buffer.scroll_segment,
		text_width,
	);

	let cursor_pos: CharIdx = buffer.cursor;
	let cursor_line = buffer.cursor_line();
	let cursor_line_start: CharIdx = buffer.doc.line_to_char(cursor_line);
	let cursor_col = cursor_pos.saturating_sub(cursor_line_start);

	let cursor_line_end: CharIdx = if cursor_line + 1 < total_lines {
		buffer.doc.line_to_char(cursor_line + 1)
	} else {
		buffer.doc.len_chars()
	};
	let cursor_line_text: String = buffer.doc.slice(cursor_line_start..cursor_line_end).into();
	let cursor_line_text = cursor_line_text.trim_end_matches('\n');
	let cursor_segments = wrap_line(cursor_line_text, text_width);
	let cursor_segment = find_segment_for_col(&cursor_segments, cursor_col);

	// Cursor is above viewport
	if cursor_line < buffer.scroll_line
		|| (cursor_line == buffer.scroll_line && cursor_segment < buffer.scroll_segment)
	{
		buffer.scroll_line = cursor_line;
		buffer.scroll_segment = cursor_segment;
		return;
	}

	// Scroll down until cursor is visible
	let mut prev_scroll = (buffer.scroll_line, buffer.scroll_segment);
	while !cursor_visible_from(
		buffer,
		buffer.scroll_line,
		buffer.scroll_segment,
		cursor_line,
		cursor_segment,
		viewport_height,
		text_width,
	) {
		scroll_viewport_down(buffer, text_width);

		let new_scroll = (buffer.scroll_line, buffer.scroll_segment);
		if new_scroll == prev_scroll {
			break;
		}
		prev_scroll = new_scroll;
	}
}

/// Clamps a segment index to valid range for a given line.
fn clamp_segment_for_line(
	buffer: &Buffer,
	line: usize,
	segment: usize,
	text_width: usize,
) -> usize {
	let total_lines = buffer.doc.len_lines();
	if line >= total_lines {
		return 0;
	}

	let line_start: CharIdx = buffer.doc.line_to_char(line);
	let line_end: CharIdx = if line + 1 < total_lines {
		buffer.doc.line_to_char(line + 1)
	} else {
		buffer.doc.len_chars()
	};

	let line_text: String = buffer.doc.slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width);
	let num_segments = segments.len().max(1);

	segment.min(num_segments.saturating_sub(1))
}

/// Finds which wrap segment contains the given column.
fn find_segment_for_col(segments: &[WrapSegment], col: usize) -> usize {
	for (i, seg) in segments.iter().enumerate() {
		let seg_end = seg.start_offset + seg.text.chars().count();
		if col < seg_end {
			return i;
		}
	}
	segments.len().saturating_sub(1)
}

/// Checks if the cursor is visible from a given viewport start position.
fn cursor_visible_from(
	buffer: &Buffer,
	start_line: usize,
	start_segment: usize,
	cursor_line: usize,
	cursor_segment: usize,
	viewport_height: usize,
	text_width: usize,
) -> bool {
	if viewport_height == 0 {
		return false;
	}

	let total_lines = buffer.doc.len_lines();
	if start_line >= total_lines {
		return false;
	}

	let mut line = start_line;
	let mut segment = clamp_segment_for_line(buffer, line, start_segment, text_width);

	for _ in 0..viewport_height {
		if line == cursor_line && segment == cursor_segment {
			return true;
		}

		if !advance_one_visual_row(buffer, &mut line, &mut segment, text_width) {
			break;
		}
	}

	false
}

/// Advances the viewport position by one visual row.
fn advance_one_visual_row(
	buffer: &Buffer,
	line: &mut usize,
	segment: &mut usize,
	text_width: usize,
) -> bool {
	let total_lines = buffer.doc.len_lines();
	if *line >= total_lines {
		return false;
	}

	let line_start: CharIdx = buffer.doc.line_to_char(*line);
	let line_end: CharIdx = if *line + 1 < total_lines {
		buffer.doc.line_to_char(*line + 1)
	} else {
		buffer.doc.len_chars()
	};

	let line_text: String = buffer.doc.slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width);
	let num_segments = segments.len().max(1);

	if *segment + 1 < num_segments {
		*segment += 1;
		return true;
	}

	if *line + 1 < total_lines {
		*line += 1;
		*segment = 0;
		return true;
	}

	false
}

/// Scrolls viewport down by one visual line.
fn scroll_viewport_down(buffer: &mut Buffer, text_width: usize) {
	let total_lines = buffer.doc.len_lines();
	if buffer.scroll_line >= total_lines {
		return;
	}

	let line_start: CharIdx = buffer.doc.line_to_char(buffer.scroll_line);
	let line_end: CharIdx = if buffer.scroll_line + 1 < total_lines {
		buffer.doc.line_to_char(buffer.scroll_line + 1)
	} else {
		buffer.doc.len_chars()
	};

	let line_text: String = buffer.doc.slice(line_start..line_end).into();
	let line_text = line_text.trim_end_matches('\n');
	let segments = wrap_line(line_text, text_width);
	let num_segments = segments.len().max(1);

	if buffer.scroll_segment + 1 < num_segments {
		buffer.scroll_segment += 1;
	} else if buffer.scroll_line + 1 < total_lines {
		buffer.scroll_line += 1;
		buffer.scroll_segment = 0;
	}
}
