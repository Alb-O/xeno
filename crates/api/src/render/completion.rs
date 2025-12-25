use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::list::ListItem;
use ratatui::widgets::{Block, Borders, List, Widget};
use tome_manifest::CompletionKind;

use crate::Editor;

impl Editor {
	pub fn render_completion_menu(&self, _area: Rect) -> impl Widget + '_ {
		let max_label_len = self
			.completions
			.items
			.iter()
			.map(|it| it.label.len())
			.max()
			.unwrap_or(0);

		let items: Vec<ListItem> = self
			.completions
			.items
			.iter()
			.enumerate()
			.map(|(i, item)| {
				let is_selected = Some(i) == self.completions.selected_idx;

				let kind_icon = match item.kind {
					CompletionKind::Command => "󰘳",
					CompletionKind::File => "󰈔",
					CompletionKind::Buffer => "󰈙",
					CompletionKind::Snippet => "󰘦",
				};

				let kind_color = match item.kind {
					CompletionKind::Command => self.theme.colors.status.command_bg,
					CompletionKind::File => self.theme.colors.status.normal_bg,
					CompletionKind::Buffer => self.theme.colors.status.view_bg,
					CompletionKind::Snippet => self.theme.colors.status.goto_bg,
				};

				let base_style = if is_selected {
					Style::default()
						.bg(self.theme.colors.ui.selection_bg)
						.fg(self.theme.colors.ui.selection_fg)
				} else {
					Style::default()
						.bg(self.theme.colors.popup.bg)
						.fg(self.theme.colors.popup.fg)
				};

				let icon_style = if is_selected {
					base_style.fg(kind_color).add_modifier(Modifier::BOLD)
				} else {
					Style::default()
						.fg(kind_color)
						.bg(self.theme.colors.popup.bg)
				};

				let label_style = if is_selected {
					base_style.add_modifier(Modifier::BOLD)
				} else {
					base_style
				};

				let kind_name = match item.kind {
					CompletionKind::Command => "Cmd",
					CompletionKind::File => "File",
					CompletionKind::Buffer => "Buf",
					CompletionKind::Snippet => "Snip",
				};

				let dim_style = if is_selected {
					base_style
				} else {
					Style::default().fg(self.theme.colors.status.dim_fg).bg(self
						.theme
						.colors
						.popup
						.bg)
				};

				let line = Line::from(vec![
					Span::styled(format!(" {} ", kind_icon), icon_style),
					Span::styled(
						format!("{:<width$}", item.label, width = max_label_len),
						label_style,
					),
					Span::styled(format!(" {:>4} ", kind_name), dim_style),
				]);

				ListItem::new(line).style(base_style)
			})
			.collect();

		let stripe_style = Style::default().fg(self.theme.colors.status.normal_bg);
		let border_set = ratatui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..ratatui::symbols::border::EMPTY
		};

		let block = Block::default()
			.style(Style::default().bg(self.theme.colors.popup.bg))
			.borders(Borders::LEFT)
			.border_set(border_set)
			.border_style(stripe_style);

		List::new(items).block(block)
	}
}
