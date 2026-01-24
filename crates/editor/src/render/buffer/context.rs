//! Buffer rendering context and cursor styling.

use std::collections::HashSet;

use unicode_width::UnicodeWidthChar;
use xeno_primitives::Mode;
use xeno_primitives::range::CharIdx;
use xeno_registry::gutter::GutterAnnotations;
use xeno_registry::themes::{SyntaxStyles, Theme};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::highlight::{HighlightSpan, HighlightStyles};
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::Paragraph;

use super::cell_style::{CellStyleInput, CursorStyleSet, resolve_cell_style};
use super::diagnostics::{DiagnosticLineMap, DiagnosticRangeMap};
use super::diff::{DiffLineNumbers, compute_diff_line_numbers, diff_line_bg};
use super::fill::FillConfig;
use super::gutter::GutterLayout;
use super::style_layers::{LineStyleContext, blend};
use crate::buffer::Buffer;
use crate::extensions::StyleOverlays;
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
	pub theme: &'a Theme,
	/// Language loader for syntax highlighting.
	pub language_loader: &'a LanguageLoader,
	/// Style overlays (e.g., zen mode dimming).
	pub style_overlays: &'a StyleOverlays,
	/// Optional diagnostic line map for gutter signs.
	pub diagnostics: Option<&'a DiagnosticLineMap>,
	/// Optional diagnostic range map for underlines.
	pub diagnostic_ranges: Option<&'a DiagnosticRangeMap>,
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

impl CursorStyles {
	/// Extracts the cursor style set for cell style resolution.
	pub fn to_cursor_set(&self) -> CursorStyleSet {
		CursorStyleSet {
			primary: self.primary,
			secondary: self.secondary,
			unfocused: self.unfocused,
		}
	}
}

impl<'a> BufferRenderContext<'a> {
	/// Creates cursor styling configuration based on theme and mode.
	pub fn make_cursor_styles(&self, mode: Mode) -> CursorStyles {
		let ui = &self.theme.colors.ui;
		let mode_color = self.mode_color(mode);

		let primary_cursor_style = Style::default()
			.bg(mode_color)
			.fg(ui.cursor_fg)
			.add_modifier(Modifier::BOLD);

		let secondary_cursor_style = {
			let bg = mode_color.blend(ui.bg, 0.4);
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

	/// Returns the background color for the given mode's status badge.
	fn mode_color(&self, mode: Mode) -> xeno_tui::style::Color {
		self.theme.colors.mode.for_mode(&mode).bg
	}

	/// Collects syntax highlight spans for a buffer's visible viewport.
	///
	/// Performs lazy syntax reparsing if the document's syntax tree is marked dirty.
	/// This is the hook point where `SyntaxPolicy::MarkDirty` edits get their
	/// syntax trees updated.
	pub fn collect_highlight_spans(
		&self,
		buffer: &Buffer,
		area: Rect,
	) -> Vec<(HighlightSpan, Style)> {
		buffer.with_doc_mut(|doc| doc.ensure_syntax_clean(self.language_loader));

		buffer.with_doc(|doc| {
			let Some(syntax) = doc.syntax() else {
				return Vec::new();
			};

			let start_line = buffer.scroll_line;
			let end_line = (start_line + area.height as usize).min(doc.content().len_lines());

			let start_byte = doc.content().line_to_byte(start_line) as u32;
			let end_byte = if end_line < doc.content().len_lines() {
				doc.content().line_to_byte(end_line) as u32
			} else {
				doc.content().len_bytes() as u32
			};

			let highlight_styles = HighlightStyles::new(SyntaxStyles::scope_names(), |scope| {
				self.theme.colors.syntax.resolve(scope)
			});

			let highlighter = syntax.highlighter(
				doc.content().slice(..),
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
		})
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

		use crate::extensions::StyleMod;

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

	/// Gets the diagnostic severity for a character position on a line.
	///
	/// Returns the highest severity if multiple diagnostics overlap at this position.
	pub fn diagnostic_severity_at(&self, line_idx: usize, char_idx: usize) -> Option<u8> {
		let spans = self.diagnostic_ranges?.get(&line_idx)?;
		let mut max_severity = 0u8;
		for span in spans {
			if char_idx >= span.start_char && char_idx < span.end_char {
				max_severity = max_severity.max(span.severity);
			}
		}
		if max_severity > 0 {
			Some(max_severity)
		} else {
			None
		}
	}

	/// Applies diagnostic underline styling to a style if the position has a diagnostic.
	///
	/// Uses colored curly underlines based on severity:
	/// - Error (4): Red curly underline
	/// - Warning (3): Yellow curly underline
	/// - Info (2): Blue curly underline
	/// - Hint (1): Cyan curly underline
	pub fn apply_diagnostic_underline(
		&self,
		line_idx: usize,
		char_idx: usize,
		style: Style,
	) -> Style {
		let Some(severity) = self.diagnostic_severity_at(line_idx, char_idx) else {
			return style;
		};

		use xeno_tui::style::UnderlineStyle;

		let underline_color = match severity {
			4 => self.theme.colors.semantic.error,
			3 => self.theme.colors.semantic.warning,
			2 => self.theme.colors.semantic.info,
			1 => self.theme.colors.semantic.hint,
			_ => return style,
		};

		style
			.underline_style(UnderlineStyle::Curl)
			.underline_color(underline_color)
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
		let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
		let has_trailing_newline = buffer.with_doc(|doc| {
			let len = doc.content().len_chars();
			len > 0 && doc.content().char(len - 1) == '\n'
		});
		let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");

		let effective_gutter = if is_diff_file {
			Self::diff_gutter_selector(gutter)
		} else {
			gutter
		};

		let diff_line_numbers: Option<Vec<DiffLineNumbers>> =
			is_diff_file.then(|| buffer.with_doc(|doc| compute_diff_line_numbers(doc.content())));

		let gutter_layout = GutterLayout::from_selector(effective_gutter, total_lines, area.width);
		let gutter_width = gutter_layout.total_width;
		let text_width = area.width.saturating_sub(gutter_width) as usize;

		let cursor = buffer.cursor;
		let ranges = buffer.selection.ranges();
		let primary_cursor = cursor;
		let cursor_heads: HashSet<CharIdx> =
			buffer.selection.ranges().iter().map(|r| r.head).collect();
		let styles = self.make_cursor_styles(buffer.mode());

		let highlight_spans = self.collect_highlight_spans(buffer, area);
		let mode_color = self.mode_color(buffer.mode());
		let base_bg = self.theme.colors.ui.bg;
		let cursor_line = buffer.cursor_line();
		let cursor_styles = styles.to_cursor_set();

		let buffer_path_owned = buffer.path();
		let buffer_path = buffer_path_owned.as_deref();

		let mut output_lines: Vec<Line> = Vec::new();
		let mut current_line_idx = buffer.scroll_line;
		let mut start_segment = buffer.scroll_segment;
		let viewport_height = area.height as usize;

		while output_lines.len() < viewport_height && current_line_idx < total_lines {
			let is_cursor_line = cursorline && current_line_idx == cursor_line;

			let diff_nums = diff_line_numbers
				.as_ref()
				.and_then(|nums| nums.get(current_line_idx));
			let line_annotations = GutterAnnotations {
				diagnostic_severity: self
					.diagnostics
					.and_then(|d| d.get(&current_line_idx).copied())
					.unwrap_or(0),
				sign: None,
				diff_old_line: diff_nums.and_then(|dn| dn.old),
				diff_new_line: diff_nums.and_then(|dn| dn.new),
			};
			let (line_start, line_end, line_text): (CharIdx, CharIdx, String) =
				buffer.with_doc(|doc| {
					let start = doc.content().line_to_char(current_line_idx);
					let end = if current_line_idx + 1 < total_lines {
						doc.content().line_to_char(current_line_idx + 1)
					} else {
						doc.content().len_chars()
					};
					let text: String = doc.content().slice(start..end).into();
					(start, end, text)
				});
			let line_text = line_text.trim_end_matches('\n');
			let line_content_end: CharIdx = line_start + line_text.chars().count();

			let line_diff_bg = diff_line_bg(is_diff_file, line_text, self.theme);
			let line_style = LineStyleContext {
				base_bg,
				diff_bg: line_diff_bg,
				mode_color,
				is_cursor_line,
				cursorline_enabled: cursorline,
				cursor_line,
				is_nontext: false,
			};

			let wrapped_segments = wrap_line(line_text, text_width, tab_width);
			let num_segments = wrapped_segments.len().max(1);

			for (seg_idx, segment) in wrapped_segments.iter().enumerate().skip(start_segment) {
				if output_lines.len() >= viewport_height {
					break;
				}

				let is_first_segment = seg_idx == 0;
				let is_last_segment = seg_idx == num_segments - 1;
				let is_continuation = !is_first_segment;

				let mut spans = buffer.with_doc(|doc| {
					gutter_layout.render_line(
						current_line_idx,
						total_lines,
						&line_style,
						is_continuation,
						doc.content().line(current_line_idx),
						buffer_path,
						&line_annotations,
						self.theme,
					)
				});

				let mut seg_col = segment.indent_cols;
				if seg_col > 0 {
					let indent_style = line_style
						.fill_bg()
						.map_or(Style::default(), |bg| Style::default().bg(bg));
					spans.push(Span::styled(" ".repeat(seg_col), indent_style));
				}

				let seg_char_offset = segment.start_offset;
				for (i, ch) in segment.text.chars().enumerate() {
					if seg_col >= text_width {
						break;
					}

					let doc_pos: CharIdx = line_start + seg_char_offset + i;
					let is_cursor = cursor_heads.contains(&doc_pos);
					let is_primary_cursor = doc_pos == primary_cursor;
					let in_selection = ranges.iter().any(|r: &xeno_primitives::range::Range| {
						doc_pos >= r.from() && doc_pos < r.to()
					});

					let byte_pos = buffer.with_doc(|doc| doc.content().char_to_byte(doc_pos));
					let syntax_style = self.style_for_byte_pos(byte_pos, &highlight_spans);
					let syntax_style = self.apply_style_overlay(byte_pos, syntax_style);

					let cell_input = CellStyleInput {
						line_ctx: &line_style,
						syntax_style,
						in_selection,
						is_primary_cursor,
						is_focused,
						cursor_styles: &cursor_styles,
						base_style: styles.base,
					};
					let resolved = resolve_cell_style(cell_input);
					let non_cursor_style = self.apply_diagnostic_underline(
						current_line_idx,
						seg_char_offset + i,
						resolved.non_cursor,
					);

					let style = if is_cursor && (use_block_cursor || !is_focused) {
						resolved.cursor
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
						spans.push(Span::styled(" ".repeat(tab_cells), style));
						seg_col += tab_cells;
					} else {
						let char_width = ch.width().unwrap_or(1).max(1);
						let remaining = text_width.saturating_sub(seg_col);
						if remaining == 0 {
							break;
						}
						if char_width <= remaining {
							spans.push(Span::styled(ch.to_string(), style));
							seg_col += char_width;
						} else {
							spans.push(Span::styled(" ".repeat(remaining), style));
							seg_col += remaining;
						}
					}
				}

				if !is_last_segment && seg_col < text_width {
					let fill_count = text_width - seg_col;
					let dim_color = self
						.theme
						.colors
						.ui
						.gutter_fg
						.blend(self.theme.colors.ui.bg, blend::GUTTER_DIM_ALPHA);
					let mut fill_style = Style::default().fg(dim_color);
					if let Some(bg) = line_style.fill_bg() {
						fill_style = fill_style.bg(bg);
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

					if cursor_at_eol && (use_block_cursor || !is_focused) {
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
						let has_newline = !is_last_doc_line || line_end > line_content_end;
						let eol_char = if has_newline { "¬" } else { " " };
						let eol_style = match (cursor_style.fg, cursor_style.bg) {
							(Some(fg), Some(bg)) => cursor_style.fg(fg.blend(bg, 0.35)),
							_ => cursor_style,
						};
						spans.push(Span::styled(eol_char, eol_style));
						seg_col += 1;
					}

					if let Some(fill_span) =
						FillConfig::from_bg(line_style.fill_bg()).fill_span(text_width - seg_col)
					{
						spans.push(fill_span);
					}
				}

				let line = if let Some(bg) = line_style.fill_bg() {
					Line::from(spans).style(Style::default().bg(bg))
				} else {
					Line::from(spans)
				};
				output_lines.push(line);
			}

			if wrapped_segments.is_empty()
				&& start_segment == 0
				&& output_lines.len() < viewport_height
			{
				let is_last_doc_line = current_line_idx + 1 >= total_lines;
				let is_phantom_line = is_last_doc_line && has_trailing_newline;

				if is_phantom_line {
					let nontext_bg = self.theme.colors.ui.nontext_bg;
					let phantom_style = LineStyleContext {
						base_bg: nontext_bg,
						diff_bg: None,
						mode_color,
						is_cursor_line: false,
						cursorline_enabled: false,
						cursor_line,
						is_nontext: true,
					};
					let mut spans = buffer.with_doc(|doc| {
						gutter_layout.render_line(
							current_line_idx,
							total_lines,
							&phantom_style,
							false,
							doc.content().line(current_line_idx),
							buffer_path,
							&line_annotations,
							self.theme,
						)
					});
					let fill_style = Style::default().bg(nontext_bg);
					spans.push(Span::styled(" ".repeat(text_width), fill_style));
					output_lines.push(Line::from(spans).style(fill_style));
				} else {
					let mut spans = buffer.with_doc(|doc| {
						gutter_layout.render_line(
							current_line_idx,
							total_lines,
							&line_style,
							false,
							doc.content().line(current_line_idx),
							buffer_path,
							&line_annotations,
							self.theme,
						)
					});

					let cursor_at_eol = cursor_heads.iter().any(|pos: &CharIdx| {
						if is_last_doc_line {
							*pos >= line_start && *pos <= line_end
						} else {
							*pos >= line_start && *pos < line_end
						}
					});
					let mut cols_used = 0;
					if cursor_at_eol && (use_block_cursor || !is_focused) {
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
						let has_newline = !is_last_doc_line || line_end > line_start;
						let eol_char = if has_newline { "¬" } else { " " };
						let eol_style = match (cursor_style.fg, cursor_style.bg) {
							(Some(fg), Some(bg)) => cursor_style.fg(fg.blend(bg, 0.35)),
							_ => cursor_style,
						};
						spans.push(Span::styled(eol_char, eol_style));
						cols_used = 1;
					}

					if let Some(fill_span) =
						FillConfig::from_bg(line_style.fill_bg()).fill_span(text_width - cols_used)
					{
						spans.push(fill_span);
					}

					let line = if let Some(bg) = line_style.fill_bg() {
						Line::from(spans).style(Style::default().bg(bg))
					} else {
						Line::from(spans)
					};
					output_lines.push(line);
				}
			}

			start_segment = 0;
			current_line_idx += 1;
		}

		let nontext_bg = self.theme.colors.ui.nontext_bg;
		while output_lines.len() < viewport_height {
			let mut spans = gutter_layout.render_empty_line(self.theme);
			spans.push(Span::styled(
				" ".repeat(text_width),
				Style::default().bg(nontext_bg),
			));
			output_lines.push(Line::from(spans).style(Style::default().bg(nontext_bg)));
		}

		RenderResult {
			widget: Paragraph::new(output_lines),
		}
	}

	/// Transforms a gutter selector for diff files by replacing standard line
	/// number gutters with `diff_line_numbers` while keeping other gutters intact.
	fn diff_gutter_selector(selector: GutterSelector) -> GutterSelector {
		static DIFF_WITH_SIGNS: &[&str] = &["diff_line_numbers", "signs"];
		static DIFF_ONLY: &[&str] = &["diff_line_numbers"];

		match selector {
			GutterSelector::Registry => GutterSelector::Named(DIFF_WITH_SIGNS),
			GutterSelector::Named(names) => {
				let has_line_nums = names.iter().any(|n| {
					matches!(
						*n,
						"line_numbers" | "relative_line_numbers" | "hybrid_line_numbers"
					)
				});
				let has_signs = names.contains(&"signs");

				match (has_line_nums, has_signs) {
					(true, true) => GutterSelector::Named(DIFF_WITH_SIGNS),
					(true, false) => GutterSelector::Named(DIFF_ONLY),
					(false, _) => GutterSelector::Named(names),
				}
			}
			other => other,
		}
	}
}
