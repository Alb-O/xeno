use std::path::PathBuf;
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
	let mut interaction = editor.state.ui.overlay_system.take_interaction();
	let opened = interaction.open(editor, Box::new(UnknownOverlay));
	editor.state.ui.overlay_system.restore_interaction(interaction);
	opened
}

#[test]
fn statusline_rows_is_one() {
	assert_eq!(STATUSLINE_ROWS, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_plan_does_not_include_overlay_tag_without_modal_overlay() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(120, 30);

	let plan = render_plan(&editor);
	assert!(!plan.iter().any(|segment| segment.text == " [Cmd]"));
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_plan_includes_dim_command_palette_tag_when_space_allows() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(200, 40);
	assert!(editor.open_command_palette());

	let plan = render_plan(&editor);
	let tag = plan
		.iter()
		.find(|segment| segment.text == " [Cmd]")
		.expect("statusline should include command tag");
	assert_eq!(tag.style, StatuslineRenderStyle::Dim);
}

#[tokio::test(flavor = "current_thread")]
async fn segment_style_maps_inverted_to_swapped_ui_colors() {
	let editor = Editor::new_scratch();
	let colors = &editor.config().theme.colors;

	let style = segment_style(&editor, StatuslineRenderStyle::Inverted);
	assert_eq!(style.fg, Some(colors.ui.bg));
	assert_eq!(style.bg, Some(colors.ui.fg));
}

#[tokio::test(flavor = "current_thread")]
async fn segment_style_uses_theme_mode_style_for_mode_segments() {
	let editor = Editor::new_scratch();
	let expected = editor.config().theme.colors.mode_style(&editor.mode());

	let style = segment_style(&editor, StatuslineRenderStyle::Mode);
	assert_eq!(style, expected);
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_file_segment_prefixes_icon_before_path_text() {
	let mut editor = Editor::new_scratch();
	let _ = editor.buffer_mut().set_path(Some(PathBuf::from("Cargo.toml")), None);

	let plan = render_plan(&editor);
	let file_segment = plan
		.iter()
		.find(|segment| segment.text.contains("Cargo.toml"))
		.expect("statusline should include file segment");

	assert!(
		!file_segment.text.starts_with(" Cargo.toml"),
		"file segment should include an icon prefix before the path"
	);
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_file_segment_uses_generic_icon_for_unknown_filetypes() {
	let mut editor = Editor::new_scratch();
	let _ = editor.buffer_mut().set_path(Some(PathBuf::from("scratch.unknown_ext_xeno")), None);

	let plan = render_plan(&editor);
	let file_segment = plan
		.iter()
		.find(|segment| segment.text.contains("scratch.unknown_ext_xeno"))
		.expect("statusline should include file segment");

	assert!(
		file_segment.text.contains("󰈔"),
		"file segment should use generic file icon when devicons returns unknown"
	);
	assert!(
		!file_segment.text.contains('*'),
		"file segment should not render devicons unknown fallback asterisk"
	);
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_command_palette_buffer_uses_named_icon_and_label() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(200, 40);
	assert!(editor.open_command_palette());

	let plan = render_plan(&editor);
	let file_segment = plan
		.iter()
		.find(|segment| segment.text.contains("[Command Palette]"))
		.expect("statusline should render command palette label while palette input is focused");

	assert!(file_segment.text.contains("󰘳"), "command palette should use command icon");
	assert!(
		!file_segment.text.contains("[No Name]"),
		"command palette should not fall back to [No Name] while focused"
	);
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_file_picker_buffer_uses_named_icon_and_label() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(200, 40);
	assert!(editor.open_file_picker());

	let plan = render_plan(&editor);
	let file_segment = plan
		.iter()
		.find(|segment| segment.text.contains("[File Picker]"))
		.expect("statusline should render file picker label while picker input is focused");

	assert!(file_segment.text.contains("󰈙"), "file picker should use file picker icon");
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_search_buffer_uses_named_icon_and_label() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(200, 40);
	assert!(editor.open_search(false));

	let plan = render_plan(&editor);
	let file_segment = plan
		.iter()
		.find(|segment| segment.text.contains("[Search]"))
		.expect("statusline should render search label while search input is focused");

	assert!(file_segment.text.contains("󰍉"), "search should use search icon");
}

#[tokio::test(flavor = "current_thread")]
async fn statusline_unknown_overlay_buffer_uses_generic_virtual_fallback_identity() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(200, 40);
	assert!(open_unknown_overlay(&mut editor));

	let plan = render_plan(&editor);
	let file_segment = plan
		.iter()
		.find(|segment| segment.text.contains("[Overlay: UnknownOverlay]"))
		.expect("statusline should render generic fallback identity for unknown overlay kinds");

	assert!(file_segment.text.contains("󰏌"), "unknown overlays should use generic virtual icon");
}
