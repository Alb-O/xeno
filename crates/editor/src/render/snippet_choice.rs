use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List, Widget};

use crate::impls::Editor;
use crate::snippet::SnippetChoiceOverlay;

impl Editor {
	pub fn render_snippet_choice_menu(&self, area: Rect, overlay: &SnippetChoiceOverlay) -> impl Widget + '_ {
		let target_width = area.width.saturating_sub(1) as usize;
		let max_option_width = overlay
			.options
			.iter()
			.map(|option| crate::render::cell_width(option))
			.max()
			.unwrap_or(1)
			.min(target_width);

		let items: Vec<ListItem> = overlay
			.options
			.iter()
			.enumerate()
			.map(|(idx, option)| {
				let is_selected = idx == overlay.selected;
				let row_style = if is_selected {
					Style::default()
						.bg(self.state.config.theme.colors.ui.selection_bg)
						.fg(self.state.config.theme.colors.ui.selection_fg)
						.add_modifier(Modifier::BOLD)
				} else {
					Style::default()
						.bg(self.state.config.theme.colors.popup.bg)
						.fg(self.state.config.theme.colors.popup.fg)
				};

				let mut line = vec![Span::styled(" ", row_style)];
				line.push(Span::styled(option.clone(), row_style));
				let width = crate::render::cell_width(option);
				if width < max_option_width {
					line.push(Span::styled(" ".repeat(max_option_width - width), row_style));
				}
				let used = 1 + max_option_width;
				if used < target_width {
					line.push(Span::styled(" ".repeat(target_width - used), row_style));
				}

				ListItem::new(Line::from(line)).style(row_style)
			})
			.collect();

		let border_set = xeno_tui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..xeno_tui::symbols::border::EMPTY
		};
		let stripe_style = Style::default().fg(self.state.config.theme.colors.mode.prefix.bg);
		let block = Block::default()
			.style(Style::default().bg(self.state.config.theme.colors.popup.bg))
			.borders(Borders::LEFT)
			.border_set(border_set)
			.border_style(stripe_style);

		List::new(items).block(block)
	}

	pub fn render_snippet_choice_popup(&self, frame: &mut xeno_tui::Frame) {
		let overlay = self.overlays().get::<SnippetChoiceOverlay>().cloned().unwrap_or_default();
		if !overlay.active || overlay.buffer_id != self.focused_view() || overlay.options.is_empty() {
			return;
		}

		let Some(buffer) = self.get_buffer(overlay.buffer_id) else {
			return;
		};
		let tab_width = self.tab_width_for(overlay.buffer_id);
		let Some((cursor_row, cursor_col)) = buffer.doc_to_screen_position(buffer.cursor, tab_width) else {
			return;
		};

		let view_area = self.focused_view_area();
		if view_area.width < 12 || view_area.height < 3 {
			return;
		}

		let max_option_width = overlay
			.options
			.iter()
			.map(|option| crate::render::cell_width(option))
			.max()
			.unwrap_or(1);
		let width = (max_option_width + 3).max(12);
		let height = overlay.options.len().clamp(1, 10);

		let mut x = view_area.x.saturating_add(cursor_col);
		let mut y = view_area.y.saturating_add(cursor_row.saturating_add(1));

		let width_u16 = width.min(view_area.width.saturating_sub(1) as usize) as u16;
		let height_u16 = height.min(view_area.height.saturating_sub(1) as usize) as u16;
		if width_u16 == 0 || height_u16 == 0 {
			return;
		}

		if x + width_u16 > view_area.right() {
			x = view_area.right().saturating_sub(width_u16);
		}
		if y + height_u16 > view_area.bottom() {
			let above = view_area.y.saturating_add(cursor_row).saturating_sub(height_u16);
			y = above.max(view_area.y);
		}

		let area = Rect::new(x, y, width_u16, height_u16);
		frame.render_widget(self.render_snippet_choice_menu(area, &overlay), area);
	}
}
