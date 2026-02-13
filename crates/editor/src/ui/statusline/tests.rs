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
