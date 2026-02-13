use super::*;

#[test]
fn statusline_plan_does_not_include_overlay_tag_without_modal_overlay() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(120, 30);

	let plan = render_plan(&editor);
	assert!(!plan.iter().any(|segment| segment.text == " [Cmd]"));
}

#[test]
fn statusline_plan_includes_dim_command_palette_tag_when_space_allows() {
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

#[test]
fn segment_style_maps_inverted_to_swapped_ui_colors() {
	let editor = Editor::new_scratch();
	let colors = &editor.config().theme.colors;

	let style = segment_style(&editor, StatuslineRenderStyle::Inverted);
	assert_eq!(style.fg, Some(colors.ui.bg));
	assert_eq!(style.bg, Some(colors.ui.fg));
}

#[test]
fn segment_style_uses_theme_mode_style_for_mode_segments() {
	let editor = Editor::new_scratch();
	let expected = editor.config().theme.colors.mode_style(&editor.mode());

	let style = segment_style(&editor, StatuslineRenderStyle::Mode);
	assert_eq!(style, expected);
}
