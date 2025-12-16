use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use tome_core::Mode;

use crate::editor::Editor;

/// A segment of a wrapped line.
pub struct WrapSegment {
    pub text: String,
    pub start_offset: usize,
}

impl Editor {
    pub fn render(&mut self, frame: &mut ratatui::Frame) {
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
            frame.render_widget(self.render_document(chunks[0]), chunks[0]);

            // Scratch buffer
            self.enter_scratch_context();
            self.ensure_cursor_visible(chunks[1]);
            let scratch_view = self.render_document(chunks[1]);
            frame.render_widget(scratch_view, chunks[1]);
            self.leave_scratch_context();

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
            frame.render_widget(self.render_document(chunks[0]), chunks[0]);
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

    pub fn render_document(&self, area: Rect) -> impl Widget + '_ {
        let total_lines = self.doc.len_lines();
        let gutter_width = self.gutter_width();
        let text_width = area.width.saturating_sub(gutter_width) as usize;

        let cursor = self.cursor;
        let primary = self.selection.primary();
        let sel_start = primary.from();
        let sel_end = primary.to();

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

            let wrapped_segments = self.wrap_line(line_text, text_width);
            let num_segments = wrapped_segments.len().max(1);

            for (seg_idx, segment) in wrapped_segments.iter().enumerate().skip(start_segment) {
                if output_lines.len() >= viewport_height {
                    break;
                }

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

                    let style = if is_cursor {
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

                if is_last_segment {
                    let line_content_end = line_start + line_text.chars().count();
                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        cursor >= line_content_end && cursor <= line_end
                    } else {
                        cursor >= line_content_end && cursor < line_end
                    };
                    if cursor_at_eol && cursor >= line_content_end {
                        spans.push(Span::styled(
                            " ",
                            Style::default()
                                .bg(Color::White)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                }

                output_lines.push(Line::from(spans));
            }

            if wrapped_segments.is_empty() && start_segment == 0
                && output_lines.len() < viewport_height {
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
                        spans.push(Span::styled(
                            " ",
                            Style::default()
                                .bg(Color::White)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD),
                        ));
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

        Paragraph::new(output_lines)
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
        let mode_style = match self.mode() {
            Mode::Normal => Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            Mode::Goto => Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            Mode::View => Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            Mode::Command { .. } => Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            Mode::PendingAction(_) => Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        };

        let modified = if self.modified { " [+]" } else { "" };
        let path = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "[scratch]".to_string());
        let cursor_info = format!(" {}:{} ", self.cursor_line() + 1, self.cursor_col() + 1);

        let count_str = if self.input.count() > 0 {
            format!(" {} ", self.input.count())
        } else {
            String::new()
        };

        let spans = vec![
            Span::styled(format!(" {} ", self.mode_name()), mode_style),
            Span::raw(count_str),
            Span::styled(
                format!(" {}{} ", path, modified),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
            Span::styled(
                cursor_info,
                Style::default().add_modifier(Modifier::REVERSED),
            ),
        ];

        Paragraph::new(Line::from(spans))
    }

    fn render_message_line(&self) -> impl Widget + '_ {
        if let Some((prompt, input)) = self.input.command_line() {
            return Paragraph::new(format!("{}{}", prompt, input))
                .style(Style::default().fg(Color::White));
        }
        let text = self.message.as_deref().unwrap_or("");
        Paragraph::new(text).style(Style::default().fg(Color::Yellow))
    }
}
