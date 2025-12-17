use crate::acp::ChatItem;
use crate::editor::Editor;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

impl Editor {
    pub fn render_agent_panel(&mut self, frame: &mut ratatui::Frame) {
        if !self.agent_panel.open {
            return;
        }

        let area = frame.area();
        let popup_height = (area.height * 40 / 100).max(10);
        let popup_area = Rect {
            x: area.x,
            y: area.height.saturating_sub(popup_height + 2), // above status/message
            width: area.width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Agent ")
            .style(
                Style::default()
                    .bg(self.theme.colors.popup.bg)
                    .fg(self.theme.colors.popup.fg),
            );

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Transcript
                Constraint::Length(3), // Input
            ])
            .split(inner);

        // Render transcript
        let mut lines = Vec::new();
        for item in &self.agent_panel.transcript {
            match item {
                ChatItem::User(text) => {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "User: ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        ratatui::text::Span::raw(text),
                    ]));
                }
                ChatItem::Assistant(text) => {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "Agent: ",
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(ratatui::style::Color::Cyan),
                        ),
                        ratatui::text::Span::raw(text),
                    ]));
                }
                ChatItem::Thought(text) => {
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "(thought) ",
                            Style::default()
                                .add_modifier(Modifier::ITALIC)
                                .fg(ratatui::style::Color::DarkGray),
                        ),
                        ratatui::text::Span::styled(
                            text,
                            Style::default().fg(ratatui::style::Color::DarkGray),
                        ),
                    ]));
                }
            }
        }

        let transcript = Paragraph::new(lines);
        frame.render_widget(transcript, chunks[0]);

        // Render input
        let input_text = self.agent_panel.input.to_string();
        let input = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::TOP).title(" Prompt "));
        frame.render_widget(input, chunks[1]);

        if self.agent_panel.focused {
            // TODO: properly handle input cursor
            frame.set_cursor_position(ratatui::layout::Position {
                x: chunks[1].x + (self.agent_panel.input_cursor % chunks[1].width as usize) as u16,
                y: chunks[1].y
                    + 1
                    + (self.agent_panel.input_cursor / chunks[1].width as usize) as u16,
            });
        }
    }
}
