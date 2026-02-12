use xeno_editor::Editor;
use xeno_editor::render::cell_width;
use xeno_editor::snippet::SnippetChoiceOverlay;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List};

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};

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
	if !visible(ed) {
		return;
	}

	let overlay = ed.overlays().get::<SnippetChoiceOverlay>().cloned().unwrap_or_default();
	if !overlay.active || overlay.buffer_id != ed.focused_view() || overlay.options.is_empty() {
		return;
	}

	let Some(buffer) = ed.get_buffer(overlay.buffer_id) else {
		return;
	};
	let tab_width = ed.tab_width_for(overlay.buffer_id);
	let Some((cursor_row, cursor_col)) = buffer.doc_to_screen_position(buffer.cursor, tab_width) else {
		return;
	};

	let view_area = ed.view_area(overlay.buffer_id);
	if view_area.width < 12 || view_area.height < 3 {
		return;
	}

	let max_option_width = overlay.options.iter().map(|option| cell_width(option)).max().unwrap_or(1);
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
