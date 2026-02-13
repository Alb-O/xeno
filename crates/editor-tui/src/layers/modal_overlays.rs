use xeno_editor::overlay::OverlayPaneRenderTarget;
use xeno_editor::window::SurfaceStyle;
use xeno_editor::{Editor, FocusTarget};
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::widgets::{Block, Borders, Paragraph};

use crate::render_adapter::to_tui_lines;

fn clamp_rect(rect: Rect, bounds: Rect) -> Option<Rect> {
	let x1 = rect.x.max(bounds.x);
	let y1 = rect.y.max(bounds.y);
	let x2 = rect.right().min(bounds.right());
	let y2 = rect.bottom().min(bounds.bottom());

	if x2 <= x1 || y2 <= y1 {
		return None;
	}

	Some(Rect {
		x: x1,
		y: y1,
		width: x2.saturating_sub(x1),
		height: y2.saturating_sub(y1),
	})
}

fn pane_content_area(rect: Rect, style: &SurfaceStyle) -> Rect {
	let mut area = rect;
	if area.width == 0 || area.height == 0 {
		return Rect::new(0, 0, 0, 0);
	}

	// Reserve one column for the docked stripe.
	if area.width > 0 {
		area.x = area.x.saturating_add(1);
		area.width = area.width.saturating_sub(1);
	}

	area.x = area.x.saturating_add(style.padding.left);
	area.y = area.y.saturating_add(style.padding.top);
	area.width = area.width.saturating_sub(style.padding.left.saturating_add(style.padding.right));
	area.height = area.height.saturating_sub(style.padding.top.saturating_add(style.padding.bottom));

	area
}

fn render_palette_completion_menu(ed: &mut Editor, frame: &mut xeno_tui::Frame, area: Rect) {
	let Some(target) = ed.overlay_completion_menu_target() else {
		return;
	};
	let menu_rect: Rect = target.rect.into();
	let Some(menu_rect) = clamp_rect(menu_rect, area) else {
		return;
	};
	crate::layers::completion::render_completion_menu(ed, frame, menu_rect, target.plan);
}

pub fn render_utility_panel_overlay(ed: &mut Editor, frame: &mut xeno_tui::Frame, area: Rect) {
	let panes: Vec<OverlayPaneRenderTarget> = ed.overlay_pane_render_plan();

	if panes.is_empty() {
		return;
	}

	let focused_overlay = match ed.focus() {
		FocusTarget::Overlay { buffer } => Some(*buffer),
		_ => None,
	};
	let stripe_fg = ed.config().theme.colors.mode.normal.bg;
	let popup_bg = ed.config().theme.colors.popup.bg;

	for pane in panes {
		let pane_rect: Rect = pane.rect.into();
		let Some(rect) = clamp_rect(pane_rect, area) else {
			continue;
		};

		let stripe_style = Style::default().fg(stripe_fg);
		let stripe_border_set = xeno_tui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..xeno_tui::symbols::border::EMPTY
		};
		let block = Block::default()
			.style(Style::default().bg(popup_bg))
			.borders(Borders::LEFT)
			.border_set(stripe_border_set)
			.border_style(stripe_style);

		let content_area = pane_content_area(rect, &pane.style);
		frame.render_widget(block, rect);

		if content_area.width == 0 || content_area.height == 0 {
			continue;
		}

		if let Some(result) = ed.buffer_view_render_plan_with_gutter(pane.buffer, content_area.into(), true, focused_overlay == Some(pane.buffer), pane.gutter)
		{
			let gutter_area = Rect {
				width: result.gutter_width,
				..content_area
			};
			let text_area = Rect {
				x: content_area.x + result.gutter_width,
				width: content_area.width.saturating_sub(result.gutter_width),
				..content_area
			};

			let gutter = to_tui_lines(result.gutter);
			let text = to_tui_lines(result.text);

			frame.render_widget(Paragraph::new(gutter), gutter_area);
			frame.render_widget(Paragraph::new(text), text_area);
		}
	}
	render_palette_completion_menu(ed, frame, area);
}
