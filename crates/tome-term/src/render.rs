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

    fn ensure_cursor_visible(&mut self, area: Rect) {
        let total_lines = self.doc.len_lines();
        let gutter_width = total_lines.max(1).ilog10() as u16 + 2;
        let text_width = area.width.saturating_sub(gutter_width) as usize;
        let viewport_height = area.height as usize;
        let cursor_line = self.cursor_line();

        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
            return;
        }

        let mut visual_rows = 0;
        let mut line_idx = self.scroll_offset;

        while line_idx < total_lines && visual_rows < viewport_height {
            let line_start = self.doc.line_to_char(line_idx);
            let line_end = if line_idx + 1 < total_lines {
                self.doc.line_to_char(line_idx + 1)
            } else {
                self.doc.len_chars()
            };

            let line_text: String = self.doc.slice(line_start..line_end).into();
            let line_text = line_text.trim_end_matches('\n');
            let wrapped = self.wrap_line(line_text, text_width);
            let rows_for_line = wrapped.len().max(1);

            if line_idx == cursor_line {
                if visual_rows + rows_for_line <= viewport_height {
                    return;
                }
                break;
            }

            visual_rows += rows_for_line;
            line_idx += 1;
        }

        if line_idx <= cursor_line {
            self.scroll_offset += 1;
            self.ensure_cursor_visible(area);
        }
    }

    pub fn render_document(&self, area: Rect) -> impl Widget + '_ {
        let total_lines = self.doc.len_lines();
        let gutter_width = total_lines.max(1).ilog10() as u16 + 2;
        let text_width = area.width.saturating_sub(gutter_width) as usize;

        let primary = self.selection.primary();
        let sel_start = primary.from();
        let sel_end = primary.to();

        let mut output_lines: Vec<Line> = Vec::new();
        let mut current_line_idx = self.scroll_offset;
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

            for (seg_idx, segment) in wrapped_segments.iter().enumerate() {
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
                for (i, ch) in segment.text.chars().enumerate() {
                    let doc_pos = line_start + seg_char_offset + i;
                    let in_selection = doc_pos >= sel_start && doc_pos < sel_end;
                    let is_cursor = doc_pos == primary.head;

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

                if is_last_segment {
                    let line_content_end = line_start + line_text.chars().count();
                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        primary.head >= line_content_end && primary.head <= line_end
                    } else {
                        primary.head >= line_content_end && primary.head < line_end
                    };
                    if cursor_at_eol && primary.head >= line_content_end {
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

            if wrapped_segments.is_empty() {
                if output_lines.len() < viewport_height {
                    let line_num_str = format!("{:>width$} ", current_line_idx + 1, width = gutter_width as usize - 1);
                    let gutter_style = Style::default().fg(Color::DarkGray);
                    let mut spans = vec![Span::styled(line_num_str, gutter_style)];

                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        primary.head >= line_start && primary.head <= line_end
                    } else {
                        primary.head >= line_start && primary.head < line_end
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
            }

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
            while pos < chars.len() && chars[pos] == ' ' {
                pos += 1;
            }
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
            Mode::Pending(_) => Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        };

        let modified = if self.modified { " [+]" } else { "" };
        let path = self.path.display().to_string();
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
