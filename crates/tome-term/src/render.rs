use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use tome_core::ext::{
    RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, render_position,
};
use tome_core::Mode;

use crate::editor::Editor;

/// A segment of a wrapped line.
pub struct WrapSegment {
    pub text: String,
    pub start_offset: usize,
}

/// Result of rendering a document, including cursor screen position.
pub struct RenderResult {
    pub widget: Paragraph<'static>,
    pub cursor_position: Option<(u16, u16)>,
}

impl Editor {
    pub fn render(&mut self, frame: &mut ratatui::Frame) {
        self.refresh_scratch_completion_hint();
        // In insert mode, we use the terminal cursor (bar), not a fake block cursor
        let use_block_cursor = !matches!(self.mode(), Mode::Insert);

        if self.scratch_open {
            let constraints = [
                Constraint::Min(1),
                Constraint::Length(self.scratch_height),
                Constraint::Length(1),
                Constraint::Length(1),
            ];
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(frame.area());

            // Main document
            self.ensure_cursor_visible(chunks[0]);
            let main_result = self.render_document_with_cursor(chunks[0], use_block_cursor && !self.scratch_focused);
            frame.render_widget(main_result.widget, chunks[0]);

            // Scratch buffer
            self.enter_scratch_context();
            self.ensure_cursor_visible(chunks[1]);
            let scratch_use_block = !matches!(self.mode(), Mode::Insert);
            let scratch_result = self.render_document_with_cursor(chunks[1], scratch_use_block && self.scratch_focused);
            frame.render_widget(scratch_result.widget, chunks[1]);

            // Set terminal cursor position for the focused buffer
            if self.scratch_focused
                && let Some((row, col)) = scratch_result.cursor_position {
                    frame.set_cursor_position(Position::new(col, row));
                }
            self.leave_scratch_context();

            if !self.scratch_focused
                && let Some((row, col)) = main_result.cursor_position {
                    frame.set_cursor_position(Position::new(col, row));
                }

            // Status line reflects focused buffer
            if self.scratch_focused {
                self.enter_scratch_context();
                let status = self.render_status_line();
                frame.render_widget(status, chunks[2]);
                self.leave_scratch_context();
            } else {
                frame.render_widget(self.render_status_line(), chunks[2]);
            }
            frame.render_widget(self.render_message_line(), chunks[3]);
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .split(frame.area());

            self.ensure_cursor_visible(chunks[0]);
            let result = self.render_document_with_cursor(chunks[0], use_block_cursor);
            frame.render_widget(result.widget, chunks[0]);

            if let Some((row, col)) = result.cursor_position {
                frame.set_cursor_position(Position::new(col, row));
            }

            frame.render_widget(self.render_status_line(), chunks[1]);
            frame.render_widget(self.render_message_line(), chunks[2]);
        }
    }

    fn ensure_cursor_visible(&mut self, area: Rect) {
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
                if line_idx == cursor_line && seg_idx == cursor_segment
                    && visual_row < viewport_height {
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
        let primary = self.selection.primary();
        let sel_start = primary.from();
        let sel_end = primary.to();

        let ghost_remainder = if self.in_scratch_context() && self.scratch_focused {
            self.scratch_completion_remainder()
        } else {
            None
        };

        let mut output_lines: Vec<Line> = Vec::new();
        let mut current_line_idx = self.scroll_line;
        let mut start_segment = self.scroll_segment;
        let viewport_height = area.height as usize;

        let mut cursor_screen_pos: Option<(u16, u16)> = None;

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

                let visual_row = output_lines.len() as u16;
                let is_first_segment = seg_idx == 0;
                let is_last_segment = seg_idx == num_segments - 1;

                let line_num_str = if is_first_segment {
                    format!("{:>width$} ", current_line_idx + 1, width = gutter_width as usize - 1)
                } else {
                    format!("{:>width$} ", "┆", width = gutter_width as usize - 1)
                };
                let gutter_style = if is_first_segment {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Rgb(60, 60, 60))
                };

                let mut spans = vec![Span::styled(line_num_str, gutter_style)];

                let seg_char_offset = segment.start_offset;
                let seg_char_count = segment.text.chars().count();
                for (i, ch) in segment.text.chars().enumerate() {
                    let doc_pos = line_start + seg_char_offset + i;
                    let in_selection = doc_pos >= sel_start && doc_pos < sel_end;
                    let is_cursor = doc_pos == cursor;

                    if is_cursor {
                        let screen_col = area.x + gutter_width + i as u16;
                        let screen_row = area.y + visual_row;
                        cursor_screen_pos = Some((screen_row, screen_col));
                    }

                    let style = if is_cursor && use_block_cursor {
                        Style::default()
                            .bg(Color::White)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else if in_selection {
                        Style::default().bg(Color::Blue).fg(Color::White)
                    } else {
                        Style::default()
                    };

                    spans.push(Span::styled(ch.to_string(), style));
                }

                if !is_last_segment && seg_char_count < text_width {
                    let fill_count = text_width - seg_char_count;
                    let fill_style = Style::default().fg(Color::Rgb(60, 60, 60));
                    spans.push(Span::styled("·".repeat(fill_count), fill_style));
                }

                let seg_start = line_start + seg_char_offset;
                let seg_end = seg_start + seg_char_count;
                let cursor_in_seg = (cursor >= seg_start && cursor <= seg_end)
                    || (is_last_segment && cursor >= line_content_end && cursor <= line_end);

                if is_last_segment {
                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        cursor >= line_content_end && cursor <= line_end
                    } else {
                        cursor >= line_content_end && cursor < line_end
                    };
                    if cursor_at_eol && cursor >= line_content_end {
                        if cursor_screen_pos.is_none() {
                            let screen_col = area.x + gutter_width + seg_char_count as u16;
                            let screen_row = area.y + visual_row;
                            cursor_screen_pos = Some((screen_row, screen_col));
                        }
                        if use_block_cursor {
                            spans.push(Span::styled(
                                " ",
                                Style::default()
                                    .bg(Color::White)
                                    .fg(Color::Black)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }
                }

                if cursor_in_seg
                    && let Some(ghost) = ghost_remainder.as_ref().filter(|g| !g.is_empty())
                {
                    spans.push(Span::styled(
                        ghost.clone(),
                        Style::default().fg(Color::Rgb(120, 120, 120)),
                    ));
                }

                output_lines.push(Line::from(spans));
            }

            if wrapped_segments.is_empty() && start_segment == 0
                && output_lines.len() < viewport_height {
                    let visual_row = output_lines.len() as u16;
                    let line_num_str = format!("{:>width$} ", current_line_idx + 1, width = gutter_width as usize - 1);
                    let gutter_style = Style::default().fg(Color::DarkGray);
                    let mut spans = vec![Span::styled(line_num_str, gutter_style)];

                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        cursor >= line_start && cursor <= line_end
                    } else {
                        cursor >= line_start && cursor < line_end
                    };
                    if cursor_at_eol {
                        let screen_col = area.x + gutter_width;
                        let screen_row = area.y + visual_row;
                        cursor_screen_pos = Some((screen_row, screen_col));

                        if use_block_cursor {
                            spans.push(Span::styled(
                                " ",
                                Style::default()
                                    .bg(Color::White)
                                    .fg(Color::Black)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }

                        if let Some(ghost) = ghost_remainder.as_ref().filter(|g| !g.is_empty()) {
                            spans.push(Span::styled(
                                ghost.clone(),
                                Style::default().fg(Color::Rgb(120, 120, 120)),
                            ));
                        }
                    }

                    output_lines.push(Line::from(spans));
                }

            start_segment = 0;
            current_line_idx += 1;
        }

        while output_lines.len() < viewport_height {
            let line_num_str = format!("{:>width$} ", "~", width = gutter_width as usize - 1);
            let gutter_style = Style::default().fg(Color::Rgb(60, 60, 60));
            output_lines.push(Line::from(vec![Span::styled(line_num_str, gutter_style)]));
        }

        RenderResult {
            widget: Paragraph::new(output_lines),
            cursor_position: cursor_screen_pos,
        }
    }

    /// Render the document (for backward compatibility with tests).
    pub fn render_document(&self, area: Rect, use_block_cursor: bool) -> impl Widget + '_ {
        self.render_document_with_cursor(area, use_block_cursor).widget
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

    fn render_status_line(&self) -> impl Widget + '_ {
        let ctx = StatuslineContext {
            mode_name: self.mode_name(),
            path: self.path.as_ref().map(|p| p.to_str().unwrap_or("[invalid path]")),
            modified: self.modified,
            line: self.cursor_line() + 1,
            col: self.cursor_col() + 1,
            count: self.input.count(),
            total_lines: self.doc.len_lines(),
            file_type: self.file_type.as_deref(),
        };

        let mut spans = Vec::new();

        // Left segments
        for seg in render_position(SegmentPosition::Left, &ctx) {
            spans.push(self.segment_to_span(&seg));
        }

        // Center segments
        for seg in render_position(SegmentPosition::Center, &ctx) {
            spans.push(self.segment_to_span(&seg));
        }

        // Right segments
        for seg in render_position(SegmentPosition::Right, &ctx) {
            spans.push(self.segment_to_span(&seg));
        }

        Paragraph::new(Line::from(spans))
    }

    fn segment_to_span(&self, segment: &RenderedSegment) -> Span<'static> {
        let style = match segment.style {
            SegmentStyle::Normal => Style::default(),
            SegmentStyle::Mode => {
                let base = match self.mode() {
                    Mode::Normal => Style::default().bg(Color::Blue).fg(Color::White),
                    Mode::Insert => Style::default().bg(Color::Green).fg(Color::Black),
                    Mode::Goto => Style::default().bg(Color::Magenta).fg(Color::White),
                    Mode::View => Style::default().bg(Color::Cyan).fg(Color::Black),
                    Mode::Command { .. } => Style::default().bg(Color::Yellow).fg(Color::Black),
                    Mode::PendingAction(_) => Style::default().bg(Color::Yellow).fg(Color::Black),
                };
                base.add_modifier(Modifier::BOLD)
            }
            SegmentStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
            SegmentStyle::Dim => Style::default().fg(Color::DarkGray),
            SegmentStyle::Warning => Style::default().fg(Color::Yellow),
            SegmentStyle::Error => Style::default().fg(Color::Red),
            SegmentStyle::Success => Style::default().fg(Color::Green),
        };
        Span::styled(segment.text.clone(), style)
    }

    fn render_message_line(&self) -> impl Widget + '_ {
        if let Some((prompt, input)) = self.input.command_line() {
            return Paragraph::new(format!("{}{}", prompt, input))
                .style(Style::default().fg(Color::White));
        }
        if let Some(msg) = self.message.as_deref() {
            return Paragraph::new(msg).style(Style::default().fg(Color::Yellow));
        }
        Paragraph::new("").style(Style::default().fg(Color::Yellow))
    }
}
