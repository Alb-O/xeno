use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use tome_cabi_types::TomeChatRole;

use crate::editor::Editor;

impl Editor {
	pub fn render_plugin_panels(&mut self, frame: &mut ratatui::Frame) {
		for panel in self.plugins.panels.values() {
			if !panel.open {
				continue;
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
				.title(format!(" {} ", panel.title))
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
			for item in &panel.transcript {
				let (prefix, style) = match item.role {
					TomeChatRole::User => ("User: ", Style::default().add_modifier(Modifier::BOLD)),
					TomeChatRole::Assistant => (
						"Assistant: ",
						Style::default()
							.add_modifier(Modifier::BOLD)
							.fg(Color::Cyan),
					),
					TomeChatRole::System => (
						"System: ",
						Style::default()
							.add_modifier(Modifier::BOLD)
							.fg(Color::Yellow),
					),
					TomeChatRole::Thought => (
						"(thought) ",
						Style::default()
							.add_modifier(Modifier::ITALIC)
							.fg(Color::DarkGray),
					),
				};

				lines.push(ratatui::text::Line::from(vec![
					ratatui::text::Span::styled(prefix, style),
					ratatui::text::Span::raw(&item.text),
				]));
			}

			let transcript = Paragraph::new(lines);
			frame.render_widget(transcript, chunks[0]);

			// Render input
			let input_text = panel.input.to_string();
			let input = Paragraph::new(input_text)
				.block(Block::default().borders(Borders::TOP).title(" Input "));
			frame.render_widget(input, chunks[1]);

			if panel.focused {
				frame.set_cursor_position(ratatui::layout::Position {
					x: chunks[1].x + (panel.input_cursor % chunks[1].width as usize) as u16,
					y: chunks[1].y + 1 + (panel.input_cursor / chunks[1].width as usize) as u16,
				});
			}
		}
	}
}
