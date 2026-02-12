use xeno_editor::Editor;
use xeno_editor::completion::CompletionState;
use xeno_tui::layout::{Constraint, Direction, Layout};
use xeno_tui::style::Style;
use xeno_tui::widgets::{Block, Clear};

use crate::layer::SceneBuilder;
use crate::scene::{SceneRenderResult, SurfaceKind, SurfaceOp};

pub fn render_frame(ed: &mut Editor, frame: &mut xeno_tui::Frame, notifications: &mut crate::layers::notifications::FrontendNotifications) {
	ed.frame_mut().needs_redraw = false;

	ed.ensure_syntax_for_buffers();

	let use_block_cursor = true;

	let area = frame.area();
	ed.viewport_mut().width = Some(area.width);
	ed.viewport_mut().height = Some(area.height);

	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints([Constraint::Min(1), Constraint::Length(1)])
		.split(area);

	let main_area = chunks[0];
	let status_area = chunks[1];

	let mut ui = std::mem::take(ed.ui_mut());
	let overlay_height = ed.overlay_interaction().active().map(|active| {
		if matches!(active.controller.name(), "CommandPalette" | "FilePicker") {
			let menu_rows = ed
				.overlays()
				.get::<CompletionState>()
				.filter(|state| state.active)
				.map_or(0u16, |state| state.visible_range().len() as u16);
			(1 + menu_rows).clamp(1, 10)
		} else if active.session.panes.len() <= 1 {
			1
		} else {
			10
		}
	});
	ui.sync_utility_for_modal_overlay(overlay_height);
	let whichkey_height = ed.whichkey_desired_height();
	ui.sync_utility_for_whichkey(whichkey_height);
	let dock_layout = ui.compute_layout(main_area.into());
	let panel_render_plan = ui.panel_render_plan(&dock_layout);
	let doc_area = dock_layout.doc_area;
	ed.viewport_mut().doc_area = Some(doc_area);
	let doc_area_tui: xeno_tui::layout::Rect = doc_area.into();

	let activate_separator_hover = {
		let layout = ed.layout();
		layout.hovered_separator.is_none() && layout.separator_under_mouse.is_some() && !layout.is_mouse_fast()
	};
	if activate_separator_hover {
		let layout = ed.layout_mut();
		let old_hover = layout.hovered_separator.take();
		layout.hovered_separator = layout.separator_under_mouse;
		if old_hover != layout.hovered_separator {
			layout.update_hover_animation(old_hover, layout.hovered_separator);
			ed.frame_mut().needs_redraw = true;
		}
	}
	if ed.layout().animation_needs_redraw() {
		ed.frame_mut().needs_redraw = true;
	}

	let ctx = ed.render_ctx();
	let doc_focused = ui.focus.focused().is_editor();

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
				let bg_block = Block::default().style(Style::default().bg(ed.config().theme.colors.ui.bg));
				frame.render_widget(bg_block, area);
			}
			SurfaceOp::Document => {
				crate::document::render_split_buffers(ed, frame, doc_area_tui, use_block_cursor && doc_focused, &ctx);
			}
			SurfaceOp::InfoPopups => crate::layers::info_popups::render(ed, frame, doc_area_tui, &ctx),
			SurfaceOp::Panels => {
				if let Some(cursor_pos) = crate::panels::render_panels(ed, frame, &panel_render_plan, &ctx) {
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
	if ui.take_wants_redraw() {
		ed.frame_mut().needs_redraw = true;
	}
	*ed.ui_mut() = ui;
}
