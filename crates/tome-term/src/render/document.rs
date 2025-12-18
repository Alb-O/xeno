use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use tome_core::Mode;

use super::terminal::ThemedVt100Terminal;
use super::types::{RenderResult, WrapSegment};
use crate::editor::Editor;
use crate::theme::blend_colors;

impl Editor {
	pub fn render(&mut self, frame: &mut ratatui::Frame) {
		// Update notifications
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.last_tick = now;
		self.notifications.tick(delta);

		// Always render block cursors (primary and secondary).
		let use_block_cursor = true;

		let area = frame.area();
		// Cache last known terminal size for input handling (e.g., mouse hit-testing)
		self.window_width = Some(area.width);
		self.window_height = Some(area.height);

		// Clear the screen to remove artifacts (e.g. terminal ghosts)
		frame.render_widget(Clear, area);

		// Set background color for the whole screen
		let bg_block = Block::default().style(Style::default().bg(self.theme.colors.ui.bg));
		frame.render_widget(bg_block, area);

		let has_command_line = self.input.command_line().is_some();
		let message_height = if has_command_line { 1 } else { 0 };

		let chunks = Layout::default()
			.direction(Direction::Vertical)
			.constraints([
				Constraint::Min(1),    // Main doc area (potentially split with terminal)
				Constraint::Length(1), // Status
				Constraint::Length(message_height), // Message/Command line
			])
			.split(area);

		let (doc_area, terminal_area) = if self.terminal_open {
			let sub = Layout::default()
				.direction(Direction::Vertical)
				.constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
				.split(chunks[0]);
			(sub[0], Some(sub[1]))
		} else {
			(chunks[0], None)
		};

		// Render main document
		// When scratch is focused, we don't draw cursor on main doc
		self.ensure_cursor_visible(doc_area);
		let main_result = self.render_document_with_cursor(
			doc_area,
			use_block_cursor && !self.scratch_focused && !self.terminal_focused,
		);
		frame.render_widget(main_result.widget, doc_area);

		// Render Terminal
		if let Some(term_area) = terminal_area {
			if let Some(term) = &mut self.terminal {
				// Resize if needed
				let (rows, cols) = term.parser.screen().size();
				if rows != term_area.height || cols != term_area.width {
					let _ = term.resize(term_area.width, term_area.height);
				}

				let screen = term.parser.screen();

				let base_style =
					Style::default()
						.bg(self.theme.colors.popup.bg)
						.fg(self.theme.colors.popup.fg);

				let term_widget = ThemedVt100Terminal::new(screen, base_style);
				frame.render_widget(term_widget, term_area);

				// Use the real terminal cursor (so the emulator draws it with the user's preferred shape).
				if self.terminal_focused && !screen.hide_cursor() {
					let (cur_row, cur_col) = screen.cursor_position();
					if cur_row < term_area.height && cur_col < term_area.width {
						frame.set_cursor_position(Position {
							x: term_area.x + cur_col,
							y: term_area.y + cur_row,
						});
					}
				}
			} else {
				// Terminal is starting: just paint the panel background.
				let style =
					Style::default()
						.bg(self.theme.colors.popup.bg)
						.fg(self.theme.colors.popup.fg);
				frame.render_widget(Block::default().style(style), term_area);
			}
		}

		// Render status line background (matches popup background)
		let status_bg = Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
		frame.render_widget(status_bg, chunks[1]);

		// Render status line content
		// We render status line based on which buffer is focused
		if self.scratch_focused {
			self.enter_scratch_context();
			let status = self.render_status_line();
			frame.render_widget(status, chunks[1]);
			self.leave_scratch_context();
		} else {
			frame.render_widget(self.render_status_line(), chunks[1]);
		}

		// Render message line if needed
		if has_command_line {
			let message_bg =
				Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
			frame.render_widget(message_bg, chunks[2]);
			frame.render_widget(self.render_message_line(), chunks[2]);
		}

		// Render Scratch Popup if open
		if self.scratch_open {
			// Command palette layout: Bottom docked (above status bar), full width, no borders
			let popup_height = 12;
			let area = frame.area();

			// Layout:
			// - Main Doc
			// - Popup (if open)
			// - Status Line
			// - Message Line

			let popup_area = Rect {
				x: area.x,
				y: area.height.saturating_sub(popup_height + 2), // +2 for status and message lines
				width: area.width,
				height: popup_height.min(area.height.saturating_sub(2)),
			};

			frame.render_widget(Clear, popup_area);

			let block = Block::default().style(
				Style::default()
					.bg(self.theme.colors.popup.bg)
					.fg(self.theme.colors.popup.fg),
			);

			let inner_area = block.inner(popup_area);
			frame.render_widget(block, popup_area);

			self.enter_scratch_context();
			// Ensure cursor visible in small area
			self.ensure_cursor_visible(inner_area);
			let scratch_use_block = !matches!(self.mode(), Mode::Insert);
			let scratch_result = self.render_document_with_cursor(inner_area, scratch_use_block);
			frame.render_widget(scratch_result.widget, inner_area);

			self.leave_scratch_context();
		}

		// Render Plugin Panels
		self.render_plugin_panels(frame);

		// Render notifications on top
		self.notifications.render(frame, frame.area());
	}

	pub fn ensure_cursor_visible(&mut self, area: Rect) {
		let total_lines = self.doc.len_lines();
		let gutter_width = self.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;
		let viewport_height = area.height as usize;

		self.text_width = text_width;

		let cursor_pos = self.cursor;
		let cursor_line = self.cursor_line();
		let cursor_line_start = self.doc.line_to_char(cursor_line);
		let cursor_col = cursor_pos.saturating_sub(cursor_line_start);

		let cursor_line_end = if cursor_line + 1 < total_lines {
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

		let mut visual_row = 0;
		let mut line_idx = self.scroll_line;
		let mut start_segment = self.scroll_segment;

		while line_idx < total_lines && visual_row < viewport_height {
			let line_start = self.doc.line_to_char(line_idx);
			let line_end = if line_idx + 1 < total_lines {
				self.doc.line_to_char(line_idx + 1)
			} else {
				self.doc.len_chars()
			};

			let line_text: String = self.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let segments = self.wrap_line(line_text, text_width);
			let num_segments = segments.len().max(1);

			for seg_idx in start_segment..num_segments {
				if line_idx == cursor_line
					&& seg_idx == cursor_segment
					&& visual_row < viewport_height
				{
					return;
				}
				visual_row += 1;
				if visual_row >= viewport_height {
					break;
				}
			}

			start_segment = 0;
			line_idx += 1;
		}

		self.scroll_down_one_visual_row(text_width);
		self.ensure_cursor_visible(area);
	}

	fn scroll_down_one_visual_row(&mut self, text_width: usize) {
		let total_lines = self.doc.len_lines();
		let line_start = self.doc.line_to_char(self.scroll_line);
		let line_end = if self.scroll_line + 1 < total_lines {
			self.doc.line_to_char(self.scroll_line + 1)
		} else {
			self.doc.len_chars()
		};

		let line_text: String = self.doc.slice(line_start..line_end).into();
		let line_text = line_text.trim_end_matches('\n');
		let segments = self.wrap_line(line_text, text_width);
		let num_segments = segments.len().max(1);

		if self.scroll_segment + 1 < num_segments {
			self.scroll_segment += 1;
		} else {
			self.scroll_line += 1;
			self.scroll_segment = 0;
		}
	}

	/// Render the document with cursor tracking.
	/// `use_block_cursor`: true = draw fake block cursor (normal mode), false = don't draw fake cursor (insert mode uses terminal cursor)
	pub fn render_document_with_cursor(&self, area: Rect, use_block_cursor: bool) -> RenderResult {
		let total_lines = self.doc.len_lines();
		let gutter_width = self.gutter_width();
		let text_width = area.width.saturating_sub(gutter_width) as usize;

		let cursor = self.cursor;
		let ranges = self.selection.ranges();
		let _primary_index = self.selection.primary_index();
		let primary_cursor = cursor;
		let cursor_heads: HashSet<usize> = ranges.iter().map(|r| r.head).collect();
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

		let mut output_lines: Vec<Line> = Vec::new();
		let mut current_line_idx = self.scroll_line;
		let mut start_segment = self.scroll_segment;
		let viewport_height = area.height as usize;

		while output_lines.len() < viewport_height && current_line_idx < total_lines {
			let line_start = self.doc.line_to_char(current_line_idx);
			let line_end = if current_line_idx + 1 < total_lines {
				self.doc.line_to_char(current_line_idx + 1)
			} else {
				self.doc.len_chars()
			};

			let line_text: String = self.doc.slice(line_start..line_end).into();
			let line_text = line_text.trim_end_matches('\n');
			let line_content_end = line_start + line_text.chars().count();

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
					let bg_color = if self.in_scratch_context() {
						self.theme.colors.popup.bg
					} else {
						self.theme.colors.ui.bg
					};
					let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
					Style::default().fg(dim_color)
				};

				let mut spans = vec![Span::styled(line_num_str, gutter_style)];

				let seg_char_offset = segment.start_offset;
				let seg_char_count = segment.text.chars().count();
				for (i, ch) in segment.text.chars().enumerate() {
					let doc_pos = line_start + seg_char_offset + i;

					let is_cursor = cursor_heads.contains(&doc_pos);
					let is_primary_cursor = doc_pos == primary_cursor;
					let in_selection = ranges
						.iter()
						.any(|r| doc_pos >= r.from() && doc_pos < r.to());

					let style = if is_cursor && use_block_cursor {
						if blink_on {
							if is_primary_cursor {
								primary_cursor_style
							} else {
								secondary_cursor_style
							}
						} else {
							base_style
						}
					} else if in_selection {
						Style::default()
							.bg(self.theme.colors.ui.selection_bg)
							.fg(self.theme.colors.ui.selection_fg)
					} else {
						base_style
					};

					spans.push(Span::styled(ch.to_string(), style));
				}

				if !is_last_segment && seg_char_count < text_width {
					let fill_count = text_width - seg_char_count;
					let bg_color = if self.in_scratch_context() {
						self.theme.colors.popup.bg
					} else {
						self.theme.colors.ui.bg
					};
					let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
					let fill_style = Style::default().fg(dim_color);
					spans.push(Span::styled("·".repeat(fill_count), fill_style));
				}

				if is_last_segment {
					let is_last_doc_line = current_line_idx + 1 >= total_lines;
					let cursor_at_eol = cursor_heads.iter().any(|pos| {
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
				let cursor_at_eol = cursor_heads.iter().any(|pos| {
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
			let bg_color = if self.in_scratch_context() {
				self.theme.colors.popup.bg
			} else {
				self.theme.colors.ui.bg
			};
			let dim_color = blend_colors(self.theme.colors.ui.gutter_fg, bg_color, 0.5);
			let gutter_style = Style::default().fg(dim_color);
			output_lines.push(Line::from(vec![Span::styled(line_num_str, gutter_style)]));
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

		let mut segments = Vec::new();
		let mut pos = 0;

		while pos < chars.len() {
			let remaining = chars.len() - pos;
			if remaining <= max_width {
				segments.push(WrapSegment {
					text: chars[pos..].iter().collect(),
					start_offset: pos,
				});
				break;
			}

			let segment_end = pos + max_width;
			let break_pos = self.find_wrap_break(&chars, pos, segment_end);

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
