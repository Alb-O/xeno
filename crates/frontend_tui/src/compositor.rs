use xeno_editor::Editor;
use xeno_tui::style::Style;
use xeno_tui::widgets::{Block, Clear};

use crate::layer::SceneBuilder;
use crate::scene::{SceneRenderResult, SurfaceKind, SurfaceOp};

pub fn render_frame(ed: &mut Editor, frame: &mut xeno_tui::Frame, notifications: &mut crate::layers::notifications::FrontendNotifications) {
	let area = frame.area();
	let viewport = xeno_editor::Rect::new(area.x, area.y, area.width, area.height);
	let frame_plan = ed.begin_frontend_frame(viewport);
	let main_area: xeno_tui::layout::Rect = frame_plan.main_area().into();
	let status_area: xeno_tui::layout::Rect = frame_plan.status_area().into();
	let doc_area_tui: xeno_tui::layout::Rect = frame_plan.doc_area().into();

	let mut builder = SceneBuilder::new(area, main_area, doc_area_tui, status_area);
	builder.push(SurfaceKind::Background, 0, area, SurfaceOp::Background, false);
	builder.push(SurfaceKind::Document, 10, doc_area_tui, SurfaceOp::Document, true);
	if crate::layers::info_popups::visible(ed) {
		crate::layers::info_popups::push(&mut builder, doc_area_tui);
	}
	builder.push(SurfaceKind::Panels, 30, main_area, SurfaceOp::Panels, false);
	if crate::layers::completion::visible(ed) {
		crate::layers::completion::push(&mut builder, doc_area_tui);
	}
	if crate::layers::snippet_choice::visible(ed) {
		crate::layers::snippet_choice::push(&mut builder, doc_area_tui);
	}
	builder.push(SurfaceKind::StatusLine, 60, status_area, SurfaceOp::StatusLine, false);
	builder.push(SurfaceKind::Notifications, 70, doc_area_tui, SurfaceOp::Notifications, false);
	let scene = builder.finish();
	let mut result = SceneRenderResult::default();

	for surface in &scene.surfaces {
		match surface.op {
			SurfaceOp::Background => {
				frame.render_widget(Clear, area);
				let bg_block = Block::default().style(Style::default().bg(ed.config().theme.colors.ui.bg.into()));
				frame.render_widget(bg_block, area);
			}
			SurfaceOp::Document => {
				crate::document::render_split_buffers(ed, frame, doc_area_tui);
			}
			SurfaceOp::InfoPopups => crate::layers::info_popups::render(ed, frame, doc_area_tui),
			SurfaceOp::Panels => {
				if let Some(cursor_pos) = crate::panels::render_panels(ed, frame, frame_plan.panel_render_plan()) {
					result.cursor = Some(cursor_pos);
				}
			}
			SurfaceOp::CompletionPopup => crate::layers::completion::render(ed, frame),
			SurfaceOp::SnippetChoicePopup => crate::layers::snippet_choice::render(ed, frame),
			SurfaceOp::StatusLine => crate::layers::status::render(ed, frame, status_area),
			SurfaceOp::Notifications => crate::layers::notifications::render(ed, notifications, doc_area_tui, frame.buffer_mut()),
		}
	}

	if let Some(cursor_pos) = result.cursor {
		frame.set_cursor_position(cursor_pos);
	}
}
