use xeno_editor::Editor;
use xeno_editor::snippet::SnippetChoiceOverlay;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List};

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};
use crate::text_width::cell_width;

fn choice_window(total: usize, selected: usize, visible_rows: usize) -> (usize, usize) {
	if total == 0 {
		return (0, 0);
	}
	let rows = visible_rows.max(1).min(total);
	if total <= rows {
		return (0, total);
	}

	let clamped_selected = selected.min(total.saturating_sub(1));
	let half = rows / 2;
	let max_start = total.saturating_sub(rows);
	let start = clamped_selected.saturating_sub(half).min(max_start);
	(start, start + rows)
}

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

	let overlay = ed.overlays().get::<SnippetChoiceOverlay>().cloned().unwrap_or_default();

	if !overlay.active || overlay.options.is_empty() {
		return;
	}
	let target_width = area.width.saturating_sub(1) as usize;
	let (window_start, window_end) = choice_window(overlay.options.len(), overlay.selected, area.height as usize);
	let selected = overlay.selected.min(overlay.options.len().saturating_sub(1));
	let max_option_width = overlay.options.iter().map(|option| cell_width(option)).max().unwrap_or(1).min(target_width);
	let theme = &ed.config().theme;

	let items: Vec<ListItem> = overlay.options[window_start..window_end]
		.iter()
		.enumerate()
		.map(|(idx, option)| {
			let absolute_idx = window_start + idx;
			let is_selected = absolute_idx == selected;
			let row_style = if is_selected {
				Style::default()
					.bg(theme.colors.ui.selection_bg)
					.fg(theme.colors.ui.selection_fg)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default().bg(theme.colors.popup.bg).fg(theme.colors.popup.fg)
			};

			let mut line = vec![Span::styled(" ", row_style)];
			line.push(Span::styled(option.clone(), row_style));
			let width = cell_width(option);
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
