//! Buffer rendering context and cursor styling.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use xeno_base::Mode;
use xeno_base::range::CharIdx;
use xeno_language::LanguageLoader;
use xeno_language::highlight::{HighlightSpan, HighlightStyles};
use xeno_registry::gutter::GutterAnnotations;
use xeno_registry::themes::{SyntaxStyles, Theme};
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::Paragraph;

use super::gutter::GutterLayout;
use crate::buffer::Buffer;
use crate::editor::extensions::StyleOverlays;
use crate::render::wrap::wrap_line;
use crate::window::GutterSelector;

/// Result of rendering a buffer's content.
pub struct RenderResult {
	/// The rendered paragraph widget ready for display.
	pub widget: Paragraph<'static>,
}

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
	/// Style for cursors in unfocused buffers (dimmed like secondary cursors).
	pub unfocused: Style,
}

/// Cursor line highlight configuration.
///
/// Separates cursor position (needed for relative line numbers) from
/// highlight enablement (can be toggled per-buffer).
#[derive(Debug, Clone, Copy)]
pub struct CursorlineConfig {
	/// Whether cursorline highlighting is enabled.
	pub enabled: bool,
	/// Background color for cursorline (mode-aware).
	pub bg: xeno_tui::style::Color,
	/// Cursor line index (real position, used for relative line numbers).
	pub line: usize,
}

impl CursorlineConfig {
	/// Returns whether a given line should have cursorline styling.
	pub fn should_highlight(&self, line_idx: usize) -> bool {
		self.enabled && line_idx == self.line
	}
}

impl<'a> BufferRenderContext<'a> {
	/// Creates cursor styling configuration based on theme and mode.
	pub fn make_cursor_styles(&self) -> CursorStyles {
		let ui = &self.theme.colors.ui;

		let primary_cursor_style = Style::default()
			.bg(ui.cursor_bg)
			.fg(ui.cursor_fg)
			.add_modifier(Modifier::BOLD);

		let secondary_cursor_style = {
			let bg = ui.cursor_bg.blend(ui.bg, 0.4);
			let fg = ui.cursor_fg.blend(ui.fg, 0.4);
			Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)
		};

		let base_style = Style::default().fg(ui.fg);

		let selection_style = Style::default().bg(ui.selection_bg).fg(ui.selection_fg);

		CursorStyles {
			primary: primary_cursor_style,
			secondary: secondary_cursor_style,
			base: base_style,
			selection: selection_style,
			unfocused: secondary_cursor_style,
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

	/// Returns the accent color for the current mode.
	fn mode_color(&self, mode: Mode) -> xeno_tui::style::Color {
		let status = &self.theme.colors.status;
		match mode {
			Mode::Normal => status.normal_bg,
			Mode::Insert => status.insert_bg,
			Mode::PendingAction(_) => status.prefix_mode_bg,
		}
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
				let xeno_tui_style: Style = abstract_style;
				(span, xeno_tui_style)
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
		use xeno_tui::animation::Animatable;

		use crate::editor::extensions::StyleMod;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to xeno_tui color for blending
				let bg: xeno_tui::style::Color = self.theme.colors.ui.bg;
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(xeno_tui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => style.fg(color),
			StyleMod::Bg(color) => style.bg(color),
		};

		Some(modified)
	}

	/// Renders a buffer into a paragraph widget using registry gutters.
	pub fn render_buffer(
		&self,
		buffer: &Buffer,
		area: Rect,
		use_block_cursor: bool,
		is_focused: bool,
		tab_width: usize,
		cursorline: bool,
	) -> RenderResult {
		self.render_buffer_with_gutter(
			buffer,
			area,
			use_block_cursor,
			is_focused,
			GutterSelector::Registry,
			tab_width,
			cursorline,
		)
	}

	/// Renders a buffer into a paragraph widget.
	///
	/// This is the main buffer rendering function that handles:
	/// - Line wrapping and viewport positioning
	/// - Cursor rendering (primary and secondary)
	/// - Selection highlighting
	/// - Gutter rendering
	/// - Cursor blinking in insert mode
	///
	/// # Parameters
	/// - `buffer`: The buffer to render
	/// - `area`: The rectangular area to render into
	/// - `use_block_cursor`: Whether to render block-style cursors
	/// - `is_focused`: Whether this buffer is the focused/active buffer
	/// - `gutter`: Gutter selection for this render pass
	/// - `tab_width`: Number of spaces a tab character occupies (from options)
	/// - `cursorline`: Whether to highlight the cursor line
	pub fn render_buffer_with_gutter(
		&self,
		buffer: &Buffer,
		area: Rect,
		use_block_cursor: bool,
		is_focused: bool,
		gutter: GutterSelector,
		tab_width: usize,
		cursorline: bool,
	) -> RenderResult {
		let total_lines = buffer.doc().content.len_lines();
		let gutter_layout = GutterLayout::from_selector(gutter, total_lines, area.width);
		let gutter_width = gutter_layout.total_width;
		let text_width = area.width.saturating_sub(gutter_width) as usize;

		let cursor = buffer.cursor;
		let ranges = buffer.selection.ranges();
		let primary_cursor = cursor;
		let cursor_heads: HashSet<CharIdx> =
			buffer.selection.ranges().iter().map(|r| r.head).collect();
		let blink_on = self.cursor_blink_visible(buffer.mode());
		let styles = self.make_cursor_styles();

		let highlight_spans = self.collect_highlight_spans(buffer, area);
		let mode_color = self.mode_color(buffer.mode());
		let cursorline_config = CursorlineConfig {
			enabled: cursorline,
			bg: self.theme.colors.ui.bg.blend(mode_color, 0.92), // 92% bg, 8% mode
			line: buffer.cursor_line(),
		};

		// Shared empty annotations for lines without diagnostic/git data
		let empty_annotations = GutterAnnotations::default();
		let buffer_path_owned = buffer.path();
		let buffer_path = buffer_path_owned.as_deref();

		let mut output_lines: Vec<Line> = Vec::new();
		let mut current_line_idx = buffer.scroll_line;
		let mut start_segment = buffer.scroll_segment;
		let viewport_height = area.height as usize;

		while output_lines.len() < viewport_height && current_line_idx < total_lines {
			let is_cursor_line = cursorline_config.should_highlight(current_line_idx);
			let line_start: CharIdx = buffer.doc().content.line_to_char(current_line_idx);
			let line_end: CharIdx = if current_line_idx + 1 < total_lines {
				buffer.doc().content.line_to_char(current_line_idx + 1)
			} else {
				buffer.doc().content.len_chars()
			};

			let line_text: String = buffer.doc().content.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let line_content_end: CharIdx = line_start + line_text.chars().count();

			let wrapped_segments = wrap_line(line_text, text_width, tab_width);
			let num_segments = wrapped_segments.len().max(1);

			for (seg_idx, segment) in wrapped_segments.iter().enumerate().skip(start_segment) {
				if output_lines.len() >= viewport_height {
					break;
				}

				let is_first_segment = seg_idx == 0;
				let is_last_segment = seg_idx == num_segments - 1;
				let is_continuation = !is_first_segment;

				let mut spans = gutter_layout.render_line(
					current_line_idx,
					total_lines,
					&cursorline_config,
					is_continuation,
					buffer.doc().content.line(current_line_idx),
					buffer_path,
					&empty_annotations,
					self.theme,
				);

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
						.any(|r: &xeno_base::range::Range| doc_pos >= r.from() && doc_pos < r.to());

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
						// Blend bg + mode color + syntax fg for selection highlight
						let base = syntax_style.unwrap_or(styles.base);
						let syntax_fg = base.fg.unwrap_or(self.theme.colors.ui.fg);
						let bg = self.theme.colors.ui.bg;
						// blend(other, alpha): alpha=1 → self, alpha=0 → other
						let selection_bg = bg
							.blend(mode_color, 0.78) // 78% bg, 22% mode
							.blend(syntax_fg, 0.88); // 88% prev, 12% syntax tint
						Style::default()
							.bg(selection_bg)
							.fg(syntax_fg)
							.add_modifier(base.add_modifier)
					} else {
						let base = syntax_style.unwrap_or(styles.base);
						if is_cursor_line && base.bg.is_none() {
							base.bg(cursorline_config.bg)
						} else {
							base
						}
					};
					let style = if is_cursor && (use_block_cursor || !is_focused) {
						if blink_on || !is_focused {
							cursor_style
						} else {
							// Blink off: show syntax-highlighted style instead of plain base
							non_cursor_style
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
					let dim_color = self
						.theme
						.colors
						.ui
						.gutter_fg
						.blend(self.theme.colors.ui.bg, 0.5);
					let mut fill_style = Style::default().fg(dim_color);
					if is_cursor_line {
						fill_style = fill_style.bg(cursorline_config.bg);
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
							Style::default().bg(cursorline_config.bg),
						));
					}
				}

				output_lines.push(Line::from(spans));
			}

			if wrapped_segments.is_empty()
				&& start_segment == 0
				&& output_lines.len() < viewport_height
			{
				let mut spans = gutter_layout.render_line(
					current_line_idx,
					total_lines,
					&cursorline_config,
					false, // not a continuation
					buffer.doc().content.line(current_line_idx),
					buffer_path,
					&empty_annotations,
					self.theme,
				);

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
						Style::default().bg(cursorline_config.bg),
					));
				}

				output_lines.push(Line::from(spans));
			}

			start_segment = 0;
			current_line_idx += 1;
		}

		while output_lines.len() < viewport_height {
			let spans = gutter_layout.render_empty_line(self.theme);
			output_lines.push(Line::from(spans));
		}

		RenderResult {
			widget: Paragraph::new(output_lines),
		}
	}
}
