use std::pin::Pin;

use super::*;
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy};
use crate::window::GutterSelector;

struct UnknownOverlay;

impl OverlayController for UnknownOverlay {
	fn name(&self) -> &'static str {
		"UnknownOverlay"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		OverlayUiSpec {
			title: Some("Unknown".to_string()),
			gutter: GutterSelector::Prompt('>'),
			rect: RectPolicy::TopCenter {
				width_percent: 100,
				max_width: u16::MAX,
				min_width: 1,
				y_frac: (1, 1),
				height: 1,
			},
			style: crate::overlay::docked_prompt_style(),
			windows: vec![],
		}
	}

	fn on_open(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession) {}

	fn on_input_changed(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _text: &str) {}

	fn on_commit<'a>(&'a mut self, _ctx: &'a mut dyn OverlayContext, _session: &'a mut OverlaySession) -> Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
		Box::pin(async {})
	}

	fn on_close(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _reason: CloseReason) {}
}

fn open_unknown_overlay(editor: &mut Editor) -> bool {
	let mut interaction = editor.state.overlay_system.take_interaction();
	let opened = interaction.open(editor, Box::new(UnknownOverlay));
	editor.state.overlay_system.restore_interaction(interaction);
	opened
}

#[test]
fn focused_document_render_plan_renders_lines_after_resize() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let plan = editor.focused_document_render_plan();
	assert!(!plan.lines.is_empty());
}

#[test]
fn focused_document_render_plan_uses_scratch_title_without_path() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.title, "[scratch]");
}

#[test]
fn focused_document_render_plan_uses_path_title_for_file_buffers() {
	let file = tempfile::NamedTempFile::new().expect("temp file");
	std::fs::write(file.path(), "alpha\n").expect("write file");

	let mut editor = Editor::new_scratch();
	let loader = editor.config().language_loader.clone();
	let _ = editor.buffer_mut().set_path(Some(file.path().to_path_buf()), Some(&loader));
	editor.handle_window_resize(80, 24);

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.title, file.path().display().to_string());
}

#[test]
fn focused_document_render_plan_uses_virtual_title_for_command_palette() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	assert!(editor.open_command_palette());

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.title, "[Command Palette]");
}

#[test]
fn focused_document_render_plan_uses_generic_virtual_title_for_unknown_overlay() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	assert!(open_unknown_overlay(&mut editor));

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.title, "[Overlay: UnknownOverlay]");
}

#[test]
fn focused_document_render_plan_returns_placeholder_for_tiny_viewport() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(1, 1);

	let plan = editor.focused_document_render_plan();
	assert_eq!(plan.lines.len(), 1);
	assert_eq!(plan.lines[0].spans[0].content.as_ref(), "document viewport too small");
}
