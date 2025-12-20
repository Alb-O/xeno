use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use tome_core::Mode;
use tome_core::range::CharIdx;

use super::types::{RenderResult, WrapSegment};
use crate::editor::Editor;
use crate::theme::blend_colors;

impl Editor {
	pub fn render(&mut self, frame: &mut ratatui::Frame) {
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.last_tick = now;
		self.notifications.tick(delta);

		let use_block_cursor = true;

		let area = frame.area();
		self.window_width = Some(area.width);
		self.window_height = Some(area.height);

		frame.render_widget(Clear, area);

		let bg_block = Block::default().style(Style::default().bg(self.theme.colors.ui.bg));
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

		// Render message line if needed (above status line)
		if has_command_line {
			let message_bg =
				Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
			frame.render_widget(message_bg, chunks[1]);
			frame.render_widget(self.render_message_line(), chunks[1]);
		}

		// Render status line background (matches popup background) - always at bottom
		let status_bg = Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
		frame.render_widget(status_bg, chunks[2]);

		// Render status line content
		// We render status line based on which buffer is focused
		frame.render_widget(self.render_status_line(), chunks[2]);

		// Render notifications within the document area with a bottom margin
		// This ensures they avoid all non-document UI and aren't flush with the bottom bar
		let mut notifications_area = doc_area;
		notifications_area.height = notifications_area.height.saturating_sub(1);
		self.notifications.render(frame, notifications_area);

		if self.completions.active {
			let max_label_len = self
				.completions
				.items
				.iter()
				.map(|it| it.label.len())
				.max()
				.unwrap_or(0);
			// 1 (left border) + 3 (icon) + max_label_len + 6 (kind label + padding)
			let menu_width = (max_label_len + 10) as u16;
			let menu_height = (self.completions.items.len() as u16).min(10);

			// Position menu above the message/command line (chunks[1])
			// This ensures it appears above both the command line and status line
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

	fn clamp_segment_for_line(&self, line: usize, segment: usize, text_width: usize) -> usize {
		let total_lines = self.doc.len_lines();
		if line >= total_lines {
			return 0;
		}

		let line_start: CharIdx = self.doc.line_to_char(line);
		let line_end: CharIdx = if line + 1 < total_lines {
			self.doc.line_to_char(line + 1)
		} else {
			self.doc.len_chars()
		};

		let line_text: String = self.doc.slice(line_start..line_end).into();
		let line_text = line_text.trim_end_matches('\n');
		let segments = self.wrap_line(line_text, text_width);
		let num_segments = segments.len().max(1);

		segment.min(num_segments.saturating_sub(1))
	}

	fn advance_one_visual_row(
		&self,
		line: &mut usize,
		segment: &mut usize,
		text_width: usize,
	) -> bool {
		let total_lines = self.doc.len_lines();
		if *line >= total_lines {
			return false;
		}

		let line_start: CharIdx = self.doc.line_to_char(*line);
		let line_end: CharIdx = if *line + 1 < total_lines {
			self.doc.line_to_char(*line + 1)
		} else {
			self.doc.len_chars()
		};

		let line_text: String = self.doc.slice(line_start..line_end).into();
		let line_text = line_text.trim_end_matches('\n');
		let segments = self.wrap_line(line_text, text_width);
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

	fn cursor_visible_from(
		&self,
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

		let total_lines = self.doc.len_lines();
		if start_line >= total_lines {
			return false;
		}

		let mut line = start_line;
		let mut segment = self.clamp_segment_for_line(line, start_segment, text_width);

		for _ in 0..viewport_height {
			if line == cursor_line && segment == cursor_segment {
				return true;
			}

			if !self.advance_one_visual_row(&mut line, &mut segment, text_width) {
				break;
			}
		}

		false
	}

	pub fn ensure_cursor_visible(&mut self, area: Rect) {
		let total_lines = self.doc.len_lines();
		let gutter_width = self.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;
		let viewport_height = area.height as usize;

		self.text_width = text_width;

		if self.scroll_line >= total_lines {
			self.scroll_line = total_lines.saturating_sub(1);
			self.scroll_segment = 0;
		}
		self.scroll_segment =
			self.clamp_segment_for_line(self.scroll_line, self.scroll_segment, text_width);

		let cursor_pos: CharIdx = self.cursor;
		let cursor_line = self.cursor_line();
		let cursor_line_start: CharIdx = self.doc.line_to_char(cursor_line);
		let cursor_col = cursor_pos.saturating_sub(cursor_line_start);

		let cursor_line_end: CharIdx = if cursor_line + 1 < total_lines {
			self.doc.line_to_char(cursor_line + 1)
		} else {
			self.doc.len_chars()
		};
		let cursor_line_text: String = self.doc.slice(cursor_line_start..cursor_line_end).into();
		let cursor_line_text = cursor_line_text.trim_end_matches('\n');
		let cursor_segments = self.wrap_line(cursor_line_text, text_width);
		let cursor_segment = self.find_segment_for_col(&cursor_segments, cursor_col);

		if cursor_line < self.scroll_line
			|| (cursor_line == self.scroll_line && cursor_segment < self.scroll_segment)
		{
			self.scroll_line = cursor_line;
			self.scroll_segment = cursor_segment;
			return;
		}

		let mut prev_scroll = (self.scroll_line, self.scroll_segment);
		while !self.cursor_visible_from(
			self.scroll_line,
			self.scroll_segment,
			cursor_line,
			cursor_segment,
			viewport_height,
			text_width,
		) {
			self.scroll_viewport_down();

			let new_scroll = (self.scroll_line, self.scroll_segment);
			if new_scroll == prev_scroll {
				break;
			}
			prev_scroll = new_scroll;
		}
	}

	/// Render the document with cursor tracking.
	/// `use_block_cursor`: true = draw fake block cursor (normal mode), false = don't draw fake cursor (insert mode uses terminal cursor)
	pub fn render_document_with_cursor(&self, area: Rect, use_block_cursor: bool) -> RenderResult {
		let total_lines = self.doc.len_lines();
		let gutter_width = self.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;
		let tab_width = 4usize;

		let cursor = self.cursor;
		let ranges = self.selection.ranges();
		let primary_cursor = cursor;
		let cursor_heads: HashSet<CharIdx> = ranges.iter().map(|r| r.head).collect();
		let insert_mode = matches!(self.mode(), Mode::Insert);
		let now_ms = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_millis();
		let blink_on = if insert_mode {
			(now_ms / 200).is_multiple_of(2)
		} else {
			true
		};

		let primary_cursor_style = Style::default()
			.bg(self.theme.colors.ui.cursor_bg)
			.fg(self.theme.colors.ui.cursor_fg)
			.add_modifier(Modifier::BOLD);
		let secondary_cursor_style = {
			let bg = blend_colors(self.theme.colors.ui.cursor_bg, self.theme.colors.ui.bg, 0.4);
			let fg = blend_colors(self.theme.colors.ui.cursor_fg, self.theme.colors.ui.fg, 0.4);
			Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)
		};
		let base_style = Style::default()
			.fg(self.theme.colors.ui.fg)
			.bg(self.theme.colors.ui.bg);
		let selection_style = Style::default()
			.bg(self.theme.colors.ui.selection_bg)
			.fg(self.theme.colors.ui.selection_fg);

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
					Style::default().fg(self.theme.colors.ui.gutter_fg)
				} else {
					let bg_color = self.theme.colors.ui.bg;
					let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
					Style::default().fg(dim_color)
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
						.any(|r: &tome_core::range::Range| doc_pos >= r.min() && doc_pos < r.max());

					let cursor_style = if is_primary_cursor {
						primary_cursor_style
					} else {
						secondary_cursor_style
					};
					let non_cursor_style = if in_selection {
						selection_style
					} else {
						base_style
					};
					let style = if is_cursor && use_block_cursor {
						if blink_on { cursor_style } else { base_style }
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
						Style::default().fg(dim_color),
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
								primary_cursor_style
							} else {
								secondary_cursor_style
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
				let gutter_style = Style::default().fg(self.theme.colors.ui.gutter_fg);
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
							primary_cursor_style
						} else {
							secondary_cursor_style
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
				Style::default().fg(dim_color),
			)]));
		}

		RenderResult {
			widget: Paragraph::new(output_lines),
		}
	}

	pub fn wrap_line(&self, line: &str, max_width: usize) -> Vec<WrapSegment> {
		if max_width == 0 {
			return vec![];
		}

		let chars: Vec<char> = line.chars().collect();
		if chars.is_empty() {
			return vec![];
		}

		// Note: this currently uses the default `tab_width` option value.
		let tab_width = 4usize;

		let mut segments = Vec::new();
		let mut pos = 0;

		while pos < chars.len() {
			let mut col = 0usize;
			let mut end = pos;

			while end < chars.len() {
				let ch = chars[end];
				let mut w = if ch == '\t' {
					tab_width.saturating_sub(col % tab_width)
				} else {
					1
				};
				if w == 0 {
					w = 1;
				}

				let remaining = max_width.saturating_sub(col);
				if remaining == 0 {
					break;
				}
				if w > remaining {
					w = remaining;
				}

				col += w;
				end += 1;
				if col >= max_width {
					break;
				}
			}

			if end == pos {
				end = (pos + 1).min(chars.len());
			}

			let break_pos = if end < chars.len() {
				let candidate = self.find_wrap_break(&chars, pos, end);
				if candidate > pos { candidate } else { end }
			} else {
				chars.len()
			};

			segments.push(WrapSegment {
				text: chars[pos..break_pos].iter().collect(),
				start_offset: pos,
			});

			pos = break_pos;
		}

		segments
	}

	fn find_wrap_break(&self, chars: &[char], start: usize, max_end: usize) -> usize {
		let search_start = start + (max_end - start) / 2;

		for i in (search_start..max_end).rev() {
			let ch = chars[i];
			if ch == ' ' || ch == '\t' {
				return i + 1;
			}
			if i + 1 < chars.len() {
				let next = chars[i + 1];
				if next == '-' || next == '/' || next == '.' || next == ',' {
					return i + 1;
				}
			}
		}

		max_end
	}
}
