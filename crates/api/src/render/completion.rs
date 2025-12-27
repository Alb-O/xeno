use tome_manifest::CompletionKind;
use tome_tui::layout::Rect;
use tome_tui::style::{Modifier, Style};
use tome_tui::text::{Line, Span};
use tome_tui::widgets::list::ListItem;
use tome_tui::widgets::{Block, Borders, List, Widget};

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

		let visible_range = self.completions.visible_range();
		let items: Vec<ListItem> = self
			.completions
			.items
			.iter()
			.enumerate()
			.filter(|(i, _)| visible_range.contains(i))
			.map(|(i, item)| {
				let is_selected = Some(i) == self.completions.selected_idx;

				let kind_icon = match item.kind {
					CompletionKind::Command => "󰘳",
					CompletionKind::File => "󰈔",
					CompletionKind::Buffer => "󰈙",
					CompletionKind::Snippet => "󰘦",
					CompletionKind::Theme => "󰏘",
				};

				let kind_color = match item.kind {
					CompletionKind::Command => self.theme.colors.status.command_bg,
					CompletionKind::File => self.theme.colors.status.normal_bg,
					CompletionKind::Buffer => self.theme.colors.status.view_bg,
					CompletionKind::Snippet => self.theme.colors.status.goto_bg,
					CompletionKind::Theme => self.theme.colors.status.view_bg,
				};

				let base_style = if is_selected {
					Style::default()
						.bg(self.theme.colors.ui.selection_bg.into())
						.fg(self.theme.colors.ui.selection_fg.into())
				} else {
					Style::default()
						.bg(self.theme.colors.popup.bg.into())
						.fg(self.theme.colors.popup.fg.into())
				};

				let icon_style = if is_selected {
					base_style
						.fg(kind_color.into())
						.add_modifier(Modifier::BOLD)
				} else {
					Style::default()
						.fg(kind_color.into())
						.bg(self.theme.colors.popup.bg.into())
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
					CompletionKind::Theme => "Theme",
				};

				let dim_style = if is_selected {
					base_style
				} else {
					Style::default()
						.fg(self.theme.colors.status.dim_fg.into())
						.bg(self.theme.colors.popup.bg.into())
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

		let stripe_style = Style::default().fg(self.theme.colors.status.normal_bg.into());
		let border_set = tome_tui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..tome_tui::symbols::border::EMPTY
		};

		let block = Block::default()
			.style(Style::default().bg(self.theme.colors.popup.bg.into()))
			.borders(Borders::LEFT)
			.border_set(border_set)
			.border_style(stripe_style);

		List::new(items).block(block)
	}
}
