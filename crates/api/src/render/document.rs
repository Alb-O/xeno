mod cursor;
mod viewport;
mod wrapping;

use std::time::{Duration, SystemTime};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use tome_base::range::CharIdx;
use tome_theme::blend_colors;

use super::types::RenderResult;
use crate::Editor;

impl Editor {
	/// Renders the complete editor frame.
	///
	/// This is the main rendering entry point that orchestrates all UI elements:
	/// - Document content with cursor and selections
	/// - UI panels (if any)
	/// - Command/message line
	/// - Status line
	/// - Notifications
	/// - Completion menu
	///
	/// # Parameters
	/// - `frame`: The ratatui frame to render into
	pub fn render(&mut self, frame: &mut ratatui::Frame) {
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.last_tick = now;
		self.notifications.tick(delta);

		// Update style overlays to reflect current cursor position.
		// This must happen at render time (not tick time) to handle
		// mouse clicks and other events that modify cursor after tick.
		self.update_style_overlays();

		let use_block_cursor = true;

		let area = frame.area();
		self.window_width = Some(area.width);
		self.window_height = Some(area.height);

		frame.render_widget(Clear, area);

		let bg_block = Block::default().style(Style::default().bg(self.theme.colors.ui.bg.into()));
		frame.render_widget(bg_block, area);

		let has_command_line = self.input.command_line().is_some();
		let message_height = if has_command_line { 1 } else { 0 };

		let chunks = Layout::default()
			.direction(Direction::Vertical)
			.constraints([
				Constraint::Min(1),
				Constraint::Length(message_height),
				Constraint::Length(1),
			])
			.split(area);

		let mut ui = std::mem::take(&mut self.ui);
		let dock_layout = ui.compute_layout(chunks[0]);
		let doc_area = dock_layout.doc_area;

		self.ensure_cursor_visible(doc_area);
		let doc_focused = ui.focus.focused().is_editor();
		let main_result =
			self.render_document_with_cursor(doc_area, use_block_cursor && doc_focused);
		frame.render_widget(main_result.widget, doc_area);

		if let Some(cursor_pos) = ui.render_panels(self, frame, &dock_layout, self.theme) {
			frame.set_cursor_position(cursor_pos);
		}
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		if has_command_line {
			let message_bg =
				Block::default().style(Style::default().bg(self.theme.colors.popup.bg.into()));
			frame.render_widget(message_bg, chunks[1]);
			frame.render_widget(self.render_message_line(), chunks[1]);

			// Position cursor at end of command line input
			if let Some((prompt, input)) = self.input.command_line() {
				let cursor_x = chunks[1].x + 1 + input.len() as u16;
				let cursor_y = chunks[1].y;
				frame.set_cursor_position((cursor_x, cursor_y));
			}
		}

		let status_bg =
			Block::default().style(Style::default().bg(self.theme.colors.popup.bg.into()));
		frame.render_widget(status_bg, chunks[2]);
		frame.render_widget(self.render_status_line(), chunks[2]);

		let mut notifications_area = doc_area;
		notifications_area.height = notifications_area.height.saturating_sub(1);
		notifications_area.width = notifications_area.width.saturating_sub(1);
		self.notifications.render(frame, notifications_area);

		if self.completions.active {
			use crate::editor::types::CompletionState;

			let max_label_len = self
				.completions
				.items
				.iter()
				.map(|it| it.label.len())
				.max()
				.unwrap_or(0);
			let menu_width = (max_label_len + 10) as u16;
			let visible_count = self.completions.items.len().min(CompletionState::MAX_VISIBLE);
			let menu_height = visible_count as u16;

			let menu_area = Rect {
				x: chunks[1].x,
				y: chunks[1].y.saturating_sub(menu_height),
				width: menu_width.min(chunks[1].width),
				height: menu_height,
			};
			frame.render_widget(Clear, menu_area);
			frame.render_widget(self.render_completion_menu(menu_area), menu_area);
		}
	}

	/// Renders the document with cursor tracking and visual effects.
	///
	/// This function handles the core document rendering logic including:
	/// - Line wrapping and viewport positioning
	/// - Cursor rendering (primary and secondary)
	/// - Selection highlighting
	/// - Gutter with line numbers
	/// - Cursor blinking in insert mode
	///
	/// # Parameters
	/// - `area`: The rectangular area to render the document into
	/// - `use_block_cursor`: Whether to render block-style cursors (normal mode)
	///   or rely on terminal cursor (insert mode)
	///
	/// # Returns
	/// A [`RenderResult`] containing the rendered paragraph widget.
	pub fn render_document_with_cursor(&self, area: Rect, use_block_cursor: bool) -> RenderResult {
		let total_lines = self.doc.len_lines();
		let gutter_width = self.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;
		let tab_width = 4usize;

		let cursor = self.cursor;
		let ranges = self.selection.ranges();
		let primary_cursor = cursor;
		let cursor_heads = self.collect_cursor_heads();
		let blink_on = self.cursor_blink_visible();
		let styles = self.make_cursor_styles();

		// Collect syntax highlight spans for the visible viewport
		let highlight_spans = self.collect_highlight_spans(area);

		let mut output_lines: Vec<Line> = Vec::new();
		let mut current_line_idx = self.scroll_line;
		let mut start_segment = self.scroll_segment;
		let viewport_height = area.height as usize;

		while output_lines.len() < viewport_height && current_line_idx < total_lines {
			let line_start: CharIdx = self.doc.line_to_char(current_line_idx);
			let line_end: CharIdx = if current_line_idx + 1 < total_lines {
				self.doc.line_to_char(current_line_idx + 1)
			} else {
				self.doc.len_chars()
			};

			let line_text: String = self.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let line_content_end: CharIdx = line_start + line_text.chars().count();

			let wrapped_segments = self.wrap_line(line_text, text_width);
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
					format!("{:>width$} ", "┆", width = gutter_width as usize - 1)
				};
				let gutter_style = if is_first_segment {
					Style::default().fg(self.theme.colors.ui.gutter_fg.into())
				} else {
					let bg_color = self.theme.colors.ui.bg;
					let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
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
					let byte_pos = self.doc.char_to_byte(doc_pos);
					let syntax_style = self.style_for_byte_pos(byte_pos, &highlight_spans);

					// Apply style overlays (e.g., zen mode dimming)
					let syntax_style = self.apply_style_overlay(byte_pos, syntax_style);

					let non_cursor_style = if in_selection {
						// Invert: syntax fg becomes bg, use contrasting color as fg
						let base = syntax_style.unwrap_or(styles.base);
						let syntax_fg = base.fg.unwrap_or(self.theme.colors.ui.fg.into());
						let text_fg = match self.theme.variant {
							tome_theme::ThemeVariant::Dark => self.theme.colors.ui.bg,
							tome_theme::ThemeVariant::Light => self.theme.colors.ui.fg,
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
					let bg_color = self.theme.colors.ui.bg;
					let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
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
			let bg_color = self.theme.colors.ui.bg;
			let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
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
