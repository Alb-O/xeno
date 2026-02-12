use xeno_tui::layout::{Constraint, Direction, Layout};
use xeno_tui::style::Style;
use xeno_tui::widgets::{Block, Clear};

use crate::impls::Editor;
use crate::ui::layer::SceneBuilder;
use crate::ui::layers;
use crate::ui::scene::{SceneRenderResult, SurfaceKind, SurfaceOp};

pub fn render_frame(ed: &mut Editor, frame: &mut xeno_tui::Frame) {
	ed.state.frame.needs_redraw = false;

	ed.ensure_syntax_for_buffers();

	let use_block_cursor = true;

	let area = frame.area();
	ed.state.viewport.width = Some(area.width);
	ed.state.viewport.height = Some(area.height);

	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints([Constraint::Min(1), Constraint::Length(1)])
		.split(area);

	let main_area = chunks[0];
	let status_area = chunks[1];

	let mut ui = std::mem::take(&mut ed.state.ui);
	let overlay_height = ed.state.overlay_system.interaction().active().map(|active| {
		if matches!(active.controller.name(), "CommandPalette" | "FilePicker") {
			let menu_rows = ed
				.overlays()
				.get::<crate::completion::CompletionState>()
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
	let whichkey_height = crate::ui::panels::utility::UtilityPanel::whichkey_desired_height(ed);
	ui.sync_utility_for_whichkey(whichkey_height);
	let dock_layout = ui.compute_layout(main_area);
	let doc_area = dock_layout.doc_area;
	ed.state.viewport.doc_area = Some(doc_area);

	if ed.state.layout.hovered_separator.is_none() && ed.state.layout.separator_under_mouse.is_some() && !ed.state.layout.is_mouse_fast() {
		let old_hover = ed.state.layout.hovered_separator.take();
		ed.state.layout.hovered_separator = ed.state.layout.separator_under_mouse;
		if old_hover != ed.state.layout.hovered_separator {
			ed.state.layout.update_hover_animation(old_hover, ed.state.layout.hovered_separator);
			ed.state.frame.needs_redraw = true;
		}
	}
	if ed.state.layout.animation_needs_redraw() {
		ed.state.frame.needs_redraw = true;
	}

	let ctx = ed.render_ctx();
	let doc_focused = ui.focus.focused().is_editor();

	let mut builder = SceneBuilder::new(area, main_area, doc_area, status_area);
	builder.push(SurfaceKind::Background, 0, area, SurfaceOp::Background, false);
	builder.push(SurfaceKind::Document, 10, doc_area, SurfaceOp::Document, true);
	if layers::info_popups::visible(ed) {
		layers::info_popups::push(&mut builder, doc_area);
	}
	builder.push(SurfaceKind::Panels, 30, main_area, SurfaceOp::Panels, false);
	if layers::completion::visible(ed) {
		layers::completion::push(&mut builder, doc_area);
	}
	if layers::snippet_choice::visible(ed) {
		layers::snippet_choice::push(&mut builder, doc_area);
	}
	builder.push(SurfaceKind::OverlayLayers, 50, area, SurfaceOp::OverlayLayers, false);
	builder.push(SurfaceKind::StatusLine, 60, status_area, SurfaceOp::StatusLine, false);
	builder.push(SurfaceKind::Notifications, 70, doc_area, SurfaceOp::Notifications, false);
	let scene = builder.finish();
	let mut result = SceneRenderResult::default();

	for surface in &scene.surfaces {
		match surface.op {
			SurfaceOp::Background => {
				frame.render_widget(Clear, area);
				let bg_block = Block::default().style(Style::default().bg(ed.state.config.theme.colors.ui.bg));
				frame.render_widget(bg_block, area);
			}
			SurfaceOp::Document => {
				ed.render_split_buffers(frame, doc_area, use_block_cursor && doc_focused, &ctx);
			}
			SurfaceOp::InfoPopups => layers::info_popups::render(ed, frame, doc_area, &ctx),
			SurfaceOp::Panels => {
				if let Some(cursor_pos) = ui.render_panels(ed, frame, &dock_layout, &ctx.theme) {
					result.cursor = Some(cursor_pos);
				}
			}
			SurfaceOp::CompletionPopup => layers::completion::render(ed, frame),
			SurfaceOp::SnippetChoicePopup => layers::snippet_choice::render(ed, frame),
			SurfaceOp::OverlayLayers => ed.state.overlay_system.layers().render(ed, frame),
			SurfaceOp::StatusLine => {
				let status_bg = Block::default().style(Style::default().bg(ctx.theme.colors.ui.bg));
				frame.render_widget(status_bg, status_area);
				frame.render_widget(ed.render_status_line(), status_area);
			}
			SurfaceOp::Notifications => {
				let mut notifications_area = doc_area;
				notifications_area.height = notifications_area.height.saturating_sub(1);
				notifications_area.width = notifications_area.width.saturating_sub(1);
				ed.state.notifications.render(notifications_area, frame.buffer_mut());
			}
		}
	}

	if let Some(cursor_pos) = result.cursor {
		frame.set_cursor_position(cursor_pos);
	}
	if ui.take_wants_redraw() {
		ed.state.frame.needs_redraw = true;
	}
	ed.state.ui = ui;
}
