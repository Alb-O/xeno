use xeno_editor::Editor;
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::widgets::{Block, Clear, Paragraph};

use crate::layer::SceneBuilder;
use crate::render_adapter::to_tui_lines;
use crate::scene::{SurfaceKind, SurfaceOp};

pub fn visible(ed: &Editor) -> bool {
	ed.info_popup_count() > 0
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::InfoPopups, 25, doc_area, SurfaceOp::InfoPopups, false);
}

pub fn render(ed: &mut Editor, frame: &mut xeno_tui::Frame, doc_area: Rect) {
	let plans = ed.info_popup_view_plans(doc_area.into());

	if plans.is_empty() {
		return;
	}

	let popup_bg = ed.config().theme.colors.popup.bg;

	for plan in &plans {
		let rect: Rect = plan.rect().into();
		if rect.width == 0 || rect.height == 0 {
			continue;
		}

		frame.render_widget(Clear, rect);

		let block = Block::default().style(Style::default().bg(popup_bg.into()));
		frame.render_widget(block, rect);

		let gutter_area: Rect = plan.gutter_rect().into();
		let text_area: Rect = plan.text_rect().into();
		if text_area.width == 0 || text_area.height == 0 {
			continue;
		}

		let gutter = to_tui_lines(plan.gutter().to_vec());
		let text = to_tui_lines(plan.text().to_vec());

		frame.render_widget(Paragraph::new(gutter), gutter_area);
		frame.render_widget(Paragraph::new(text), text_area);
	}
}
