use xeno_editor::Editor;
use xeno_editor::window::GutterSelector;
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::widgets::block::Padding;
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
	let targets = ed.info_popup_layout_plan(doc_area.into());

	if targets.is_empty() {
		return;
	}

	let popup_bg = ed.config().theme.colors.popup.bg;
	let padding = Padding::horizontal(1);

	for target in targets {
		let rect: Rect = target.rect.into();
		if rect.width == 0 || rect.height == 0 {
			continue;
		}

		frame.render_widget(Clear, rect);

		let block = Block::default().style(Style::default().bg(popup_bg)).padding(padding);

		let inner = block.inner(rect);
		frame.render_widget(block, rect);

		if inner.width == 0 || inner.height == 0 {
			continue;
		}

		let Some(result) = ed.buffer_view_render_plan_with_gutter(target.buffer_id, inner.into(), false, false, GutterSelector::Hidden) else {
			continue;
		};

		let gutter_area = Rect {
			width: result.gutter_width,
			..inner
		};
		let text_area = Rect {
			x: inner.x + result.gutter_width,
			width: inner.width.saturating_sub(result.gutter_width),
			..inner
		};

		let gutter = to_tui_lines(result.gutter);
		let text = to_tui_lines(result.text);

		frame.render_widget(Paragraph::new(gutter), gutter_area);
		frame.render_widget(Paragraph::new(text), text_area);
	}
}
