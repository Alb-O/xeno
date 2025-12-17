use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Widget};

use crate::editor::{Editor, MessageKind};

impl Editor {
    pub fn render_message_line(&self) -> impl Widget + '_ {
        if let Some((prompt, input)) = self.input.command_line() {
            return Paragraph::new(format!("{}{}", prompt, input))
                .style(Style::default().fg(self.theme.colors.ui.command_input_fg));
        }
        if let Some(msg) = &self.message {
            let color = match msg.kind {
                MessageKind::Info => self.theme.colors.ui.message_fg,
                MessageKind::Error => self.theme.colors.status.error_fg,
            };
            return Paragraph::new(msg.text.as_str()).style(Style::default().fg(color));
        }
        Paragraph::new("").style(Style::default().fg(self.theme.colors.ui.message_fg))
    }
}
