use super::*;

#[test]
fn snippet_choice_render_plan_marks_selected_item() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let buffer_id = editor.focused_view();

	*editor.overlays_mut().get_or_default::<SnippetChoiceOverlay>() = SnippetChoiceOverlay {
		active: true,
		buffer_id,
		tabstop_idx: 1,
		options: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
		selected: 1,
	};

	let plan = editor.snippet_choice_render_plan().expect("plan should exist");
	assert_eq!(plan.items.len(), 3);
	assert_eq!(plan.items[1].option, "beta");
	assert!(plan.items[1].selected);
	assert_eq!(plan.max_option_width, "alpha".width());
}

#[test]
fn snippet_choice_render_plan_is_hidden_while_modal_overlay_is_open() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let buffer_id = editor.focused_view();

	*editor.overlays_mut().get_or_default::<SnippetChoiceOverlay>() = SnippetChoiceOverlay {
		active: true,
		buffer_id,
		tabstop_idx: 1,
		options: vec!["one".to_string(), "two".to_string()],
		selected: 0,
	};

	assert!(editor.snippet_choice_render_plan().is_some());
	assert!(editor.open_command_palette());
	assert!(editor.snippet_choice_render_plan().is_none());
}
