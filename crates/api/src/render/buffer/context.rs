//! Buffer rendering context and cursor styling.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use evildoer_base::range::CharIdx;
use evildoer_language::LanguageLoader;
use evildoer_language::highlight::{HighlightSpan, HighlightStyles};
use evildoer_manifest::syntax::SyntaxStyles;
use evildoer_manifest::theme::IndentGuideChars;
use evildoer_manifest::{Mode, Theme, ThemeVariant};
use evildoer_tui::layout::Rect;
use evildoer_tui::style::{Modifier, Style};
use evildoer_tui::text::{Line, Span};
use evildoer_tui::widgets::Paragraph;

use crate::buffer::Buffer;
use crate::editor::extensions::StyleOverlays;
use crate::render::types::{RenderResult, wrap_line};

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
	/// Whether to show indent guide characters for leading whitespace.
	pub show_indent_guides: bool,
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
	/// Style for cursors in unfocused buffers (dimmed like secondary cursors).
	pub unfocused: Style,
}

impl<'a> BufferRenderContext<'a> {
	/// Creates cursor styling configuration based on theme and mode.
	pub fn make_cursor_styles(&self) -> CursorStyles {
		let primary_cursor_style = Style::default()
			.bg(self.theme.colors.ui.cursor_bg.into())
			.fg(self.theme.colors.ui.cursor_fg.into())
			.add_modifier(Modifier::BOLD);

		let secondary_cursor_style = {
			let bg = self
				.theme
				.colors
				.ui
				.cursor_bg
				.blend(self.theme.colors.ui.bg, 0.4);
			let fg = self
				.theme
				.colors
				.ui
				.cursor_fg
				.blend(self.theme.colors.ui.fg, 0.4);
			Style::default()
				.bg(bg.into())
				.fg(fg.into())
				.add_modifier(Modifier::BOLD)
		};

		let base_style = Style::default().fg(self.theme.colors.ui.fg.into());

		let selection_style = Style::default()
			.bg(self.theme.colors.ui.selection_bg.into())
			.fg(self.theme.colors.ui.selection_fg.into());

		CursorStyles {
			primary: primary_cursor_style,
			secondary: secondary_cursor_style,
			base: base_style,
			selection: selection_style,
			unfocused: secondary_cursor_style,
		}
	}

	/// Returns the style for indent guide characters.
	///
	/// Uses the theme's `indent_guide_fg` if set, otherwise derives a subtle
	/// color by blending `gutter_fg` with the background.
	pub fn indent_guide_style(&self) -> Style {
		let fg = self
			.theme
			.colors
			.ui
			.indent_guide_fg
			.unwrap_or_else(|| self.theme.colors.ui.gutter_fg.blend(self.theme.colors.ui.bg, 0.6));
		Style::default().fg(fg.into())
	}

	/// Returns the characters used for indent guide rendering.
	pub fn indent_guide_chars(&self) -> IndentGuideChars {
		IndentGuideChars::default()
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
		let doc = buffer.doc();
		let Some(ref syntax) = doc.syntax else {
			return Vec::new();
		};

		let start_line = buffer.scroll_line;
		let end_line = (start_line + area.height as usize).min(doc.content.len_lines());

		let start_byte = doc.content.line_to_byte(start_line) as u32;
		let end_byte = if end_line < doc.content.len_lines() {
			doc.content.line_to_byte(end_line) as u32
		} else {
			doc.content.len_bytes() as u32
		};

		let highlight_styles = HighlightStyles::new(SyntaxStyles::scope_names(), |scope| {
			self.theme.colors.syntax.resolve(scope)
		});

		let highlighter = syntax.highlighter(
			doc.content.slice(..),
			self.language_loader,
			start_byte..end_byte,
		);

		highlighter
			.map(|span| {
				let abstract_style = highlight_styles.style_for_highlight(span.highlight);
				let evildoer_tui_style: Style = abstract_style.into();
				(span, evildoer_tui_style)
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
		use evildoer_tui::animation::Animatable;

		use crate::editor::extensions::StyleMod;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to evildoer_tui color for blending
				let bg: evildoer_tui::style::Color = self.theme.colors.ui.bg.into();
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(evildoer_tui::style::Color::DarkGray)
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
	/// - `is_focused`: Whether this buffer is the focused/active buffer
	pub fn render_buffer(
		&self,
		buffer: &Buffer,
		area: Rect,
		use_block_cursor: bool,
		is_focused: bool,
	) -> RenderResult {
		let total_lines = buffer.doc().content.len_lines();
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

		let highlight_spans = self.collect_highlight_spans(buffer, area);
		let cursor_line = buffer.cursor_line();
		let cursorline_bg: evildoer_tui::style::Color = self.theme.colors.ui.cursorline_bg.into();

		let mut output_lines: Vec<Line> = Vec::new();
		let mut current_line_idx = buffer.scroll_line;
		let mut start_segment = buffer.scroll_segment;
		let viewport_height = area.height as usize;

		while output_lines.len() < viewport_height && current_line_idx < total_lines {
			let is_cursor_line = current_line_idx == cursor_line;
			let line_start: CharIdx = buffer.doc().content.line_to_char(current_line_idx);
			let line_end: CharIdx = if current_line_idx + 1 < total_lines {
				buffer.doc().content.line_to_char(current_line_idx + 1)
			} else {
				buffer.doc().content.len_chars()
			};

			let line_text: String = buffer.doc().content.slice(line_start..line_end).into();
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
					let style = Style::default().fg(self.theme.colors.ui.gutter_fg.into());
					if is_cursor_line {
						style.bg(cursorline_bg)
					} else {
						style
					}
				} else {
					let dim_color = self
						.theme
						.colors
						.ui
						.gutter_fg
						.blend(self.theme.colors.ui.bg, 0.5);
					let style = Style::default().fg(dim_color.into());
					if is_cursor_line {
						style.bg(cursorline_bg)
					} else {
						style
					}
				};

				let mut spans = vec![Span::styled(line_num_str, gutter_style)];

				// Track whether we're still in leading indentation for this segment.
				// Only the first segment of a line can have leading indentation.
				let mut in_leading_indent = is_first_segment;

				// Prepare indent guide style and characters if enabled
				let indent_guide_style = if self.show_indent_guides {
					Some(self.indent_guide_style())
				} else {
					None
				};
				let indent_chars = self.indent_guide_chars();

				let seg_char_offset = segment.start_offset;
				let mut seg_col = 0usize;
				for (i, ch) in segment.text.chars().enumerate() {
					if seg_col >= text_width {
						break;
					}

					let doc_pos: CharIdx = line_start + seg_char_offset + i;

					// Check if this character ends leading indentation
					if in_leading_indent && ch != ' ' && ch != '\t' {
						in_leading_indent = false;
					}

					let is_cursor = cursor_heads.contains(&doc_pos);
					let is_primary_cursor = doc_pos == primary_cursor;
					let in_selection = ranges.iter().any(|r: &evildoer_base::range::Range| {
						doc_pos >= r.min() && doc_pos < r.max()
					});

					let cursor_style = if !is_focused {
						styles.unfocused
					} else if is_primary_cursor {
						styles.primary
					} else {
						styles.secondary
					};

					// Convert char position to byte position for highlight lookup
					let byte_pos = buffer.doc().content.char_to_byte(doc_pos);
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
						let base = syntax_style.unwrap_or(styles.base);
						if is_cursor_line && base.bg.is_none() {
							base.bg(cursorline_bg)
						} else {
							base
						}
					};
					let style = if is_cursor && (use_block_cursor || !is_focused) {
						if blink_on || !is_focused {
							cursor_style
						} else {
							styles.base
						}
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
						} else if in_leading_indent && let Some(guide_style) = indent_guide_style {
							// Render tab with chevron indicator at start, spaces for rest
							let guide_style_with_bg = if is_cursor_line && !in_selection {
								guide_style.bg(cursorline_bg)
							} else {
								guide_style
							};
							spans.push(Span::styled(
								indent_chars.tab.to_string(),
								guide_style_with_bg,
							));
							if tab_cells > 1 {
								spans.push(Span::styled(" ".repeat(tab_cells - 1), style));
							}
						} else {
							spans.push(Span::styled(" ".repeat(tab_cells), style));
						}

						seg_col += tab_cells;
					} else if ch == ' ' && in_leading_indent && let Some(guide_style) = indent_guide_style {
						// Render space with dot indicator for leading indentation
						let guide_style_with_bg = if is_cursor_line && !in_selection {
							guide_style.bg(cursorline_bg)
						} else {
							guide_style
						};
						spans.push(Span::styled(
							indent_chars.space.to_string(),
							guide_style_with_bg,
						));
						seg_col += 1;
					} else {
						spans.push(Span::styled(ch.to_string(), style));
						seg_col += 1;
					}
				}

				if !is_last_segment && seg_col < text_width {
					let fill_count = text_width - seg_col;
					let dim_color = self
						.theme
						.colors
						.ui
						.gutter_fg
						.blend(self.theme.colors.ui.bg, 0.5);
					let mut fill_style = Style::default().fg(dim_color.into());
					if is_cursor_line {
						fill_style = fill_style.bg(cursorline_bg);
					}
					spans.push(Span::styled(" ".repeat(fill_count), fill_style));
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

					if cursor_at_eol && ((use_block_cursor && blink_on) || !is_focused) {
						let primary_here = if is_last_doc_line {
							primary_cursor >= line_content_end && primary_cursor <= line_end
						} else {
							primary_cursor >= line_content_end && primary_cursor < line_end
						};
						let cursor_style = if !is_focused {
							styles.unfocused
						} else if primary_here {
							styles.primary
						} else {
							styles.secondary
						};
						spans.push(Span::styled(" ", cursor_style));
						seg_col += 1;
					}

					if is_cursor_line && seg_col < text_width {
						spans.push(Span::styled(
							" ".repeat(text_width - seg_col),
							Style::default().bg(cursorline_bg),
						));
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
				let mut gutter_style = Style::default().fg(self.theme.colors.ui.gutter_fg.into());
				if is_cursor_line {
					gutter_style = gutter_style.bg(cursorline_bg);
				}
				let mut spans = vec![Span::styled(line_num_str, gutter_style)];

				let is_last_doc_line = current_line_idx + 1 >= total_lines;
				let cursor_at_eol = cursor_heads.iter().any(|pos: &CharIdx| {
					if is_last_doc_line {
						*pos >= line_start && *pos <= line_end
					} else {
						*pos >= line_start && *pos < line_end
					}
				});
				let mut cols_used = 0;
				if cursor_at_eol && ((use_block_cursor && blink_on) || !is_focused) {
					let primary_here = if is_last_doc_line {
						primary_cursor >= line_start && primary_cursor <= line_end
					} else {
						primary_cursor >= line_start && primary_cursor < line_end
					};
					let cursor_style = if !is_focused {
						styles.unfocused
					} else if primary_here {
						styles.primary
					} else {
						styles.secondary
					};
					spans.push(Span::styled(" ", cursor_style));
					cols_used = 1;
				}

				if is_cursor_line && cols_used < text_width {
					spans.push(Span::styled(
						" ".repeat(text_width - cols_used),
						Style::default().bg(cursorline_bg),
					));
				}

				output_lines.push(Line::from(spans));
			}

			start_segment = 0;
			current_line_idx += 1;
		}

		while output_lines.len() < viewport_height {
			let line_num_str = format!("{:>width$} ", "~", width = gutter_width as usize - 1);
			let dim_color = self
				.theme
				.colors
				.ui
				.gutter_fg
				.blend(self.theme.colors.ui.bg, 0.5);
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
