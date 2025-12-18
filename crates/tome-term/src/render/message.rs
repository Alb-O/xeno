use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Widget};

use crate::editor::Editor;

impl Editor {
	pub fn render_message_line(&self) -> impl Widget + '_ {
		if let Some((prompt, input)) = self.input.command_line() {
			return Paragraph::new(format!("{}{}", prompt, input))
				.style(Style::default().fg(self.theme.colors.ui.command_input_fg));
		}
		Paragraph::new("").style(Style::default().fg(self.theme.colors.ui.message_fg))
	}
}
