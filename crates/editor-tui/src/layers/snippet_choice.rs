use xeno_editor::Editor;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List};

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};
use crate::text_width::cell_width;
pub fn visible(ed: &Editor) -> bool {
	ed.snippet_choice_popup_visible()
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::SnippetChoicePopup, 45, doc_area, SurfaceOp::SnippetChoicePopup, false);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	let Some(area) = ed.snippet_choice_popup_area() else {
		return;
	};
	let area: Rect = area.into();
	let Some(plan) = ed.snippet_choice_render_plan() else {
		return;
	};
	let target_width = plan.target_row_width();
	let max_option_width = plan.max_option_width();
	let theme = &ed.config().theme;

	let items: Vec<ListItem> = plan
		.items()
		.iter()
		.map(|item| {
			let is_selected = item.selected();
			let option = item.option();
			let row_style = if is_selected {
				Style::default()
					.bg(theme.colors.ui.selection_bg)
					.fg(theme.colors.ui.selection_fg)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default().bg(theme.colors.popup.bg).fg(theme.colors.popup.fg)
			};

			let mut line = vec![Span::styled(" ", row_style)];
			let width = cell_width(option);
			line.push(Span::styled(option.to_string(), row_style));
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
	let stripe_style = Style::default().fg(theme.colors.mode.prefix.bg);
	let block = Block::default()
		.style(Style::default().bg(theme.colors.popup.bg))
		.borders(Borders::LEFT)
		.border_set(border_set)
		.border_style(stripe_style);
	frame.render_widget(List::new(items).block(block), area);
}
