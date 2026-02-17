use super::*;
use crate::completion::{CompletionFileMeta, CompletionItem};

fn item(label: &str, kind: CompletionKind) -> CompletionItem {
	CompletionItem {
		label: label.to_string(),
		insert_text: label.to_string(),
		detail: None,
		filter_text: None,
		kind,
		match_indices: None,
		right: Some("meta".to_string()),
		file: None,
	}
}

#[test]
fn completion_render_plan_keeps_selected_item_visible_with_limit() {
	let mut editor = Editor::new_scratch();

	let state = editor.overlays_mut().get_or_default::<CompletionState>();
	state.active = true;
	state.items = vec![
		item("item0", CompletionKind::Command),
		item("item1", CompletionKind::Command),
		item("item2", CompletionKind::Command),
		item("item3", CompletionKind::Command),
		item("item4", CompletionKind::Command),
	];
	state.selected_idx = Some(3);
	state.scroll_offset = 0;
	state.show_kind = true;

	let plan = editor.completion_render_plan(40, 2).expect("plan should exist");
	assert_eq!(plan.items.len(), 2);
	assert_eq!(plan.items[0].label, "item2");
	assert_eq!(plan.items[1].label, "item3");
	assert!(!plan.items[0].selected);
	assert!(plan.items[1].selected);
}

#[test]
fn completion_render_plan_applies_width_column_policy() {
	let mut editor = Editor::new_scratch();

	let state = editor.overlays_mut().get_or_default::<CompletionState>();
	state.active = true;
	state.items = vec![item("entry", CompletionKind::File)];
	state.selected_idx = Some(0);
	state.show_kind = true;

	let narrow = editor.completion_render_plan(20, 10).expect("plan should exist");
	assert!(!narrow.show_kind);
	assert!(!narrow.show_right);

	let state = editor.overlays_mut().get_or_default::<CompletionState>();
	state.show_kind = false;

	let wide = editor.completion_render_plan(31, 10).expect("plan should exist");
	assert!(!wide.show_kind);
	assert!(wide.show_right);
}

#[test]
fn completion_render_plan_includes_file_presentation_payload() {
	let mut editor = Editor::new_scratch();

	let state = editor.overlays_mut().get_or_default::<CompletionState>();
	state.active = true;
	state.items = vec![CompletionItem {
		label: String::from("somefile.unknown_xeno_ext"),
		insert_text: String::from("somefile.unknown_xeno_ext"),
		detail: Some(String::from("file")),
		filter_text: None,
		kind: CompletionKind::File,
		match_indices: None,
		right: Some(String::from("file")),
		file: Some(CompletionFileMeta::new("somefile.unknown_xeno_ext", xeno_buffer_display::FileKind::File)),
	}];
	state.selected_idx = Some(0);

	let plan = editor.completion_render_plan(40, 10).expect("plan should exist");
	let row = plan.items.first().expect("file row should exist");
	let presentation = row.file_presentation().expect("file presentation should be set");

	assert_eq!(presentation.icon(), "󰈔");
	assert_eq!(presentation.label(), "somefile.unknown_xeno_ext");
	assert!(!presentation.icon().contains('*'));
}

#[test]
fn overlay_menu_rect_returns_none_when_no_space_above_input() {
	let input = Rect::new(10, 12, 30, 1);
	assert_eq!(overlay_menu_rect_above_input(12, input, 5), None);
}

#[test]
fn overlay_menu_rect_clamps_height_to_available_rows() {
	let input = Rect::new(10, 15, 30, 1);
	let menu = overlay_menu_rect_above_input(10, input, 20).expect("menu should exist");
	assert_eq!(menu.y, 10);
	assert_eq!(menu.height, 5);
}

#[test]
fn overlay_menu_rect_returns_none_for_zero_width_or_rows() {
	assert_eq!(overlay_menu_rect_above_input(0, Rect::new(10, 5, 0, 1), 4), None);
	assert_eq!(overlay_menu_rect_above_input(0, Rect::new(10, 5, 20, 1), 0), None);
}

#[test]
fn overlay_completion_menu_target_uses_utility_panel_bounds() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	assert!(editor.open_command_palette());

	let completion_count = 16;
	let state = editor.overlays_mut().get_or_default::<CompletionState>();
	state.active = true;
	state.items = (0..completion_count).map(|idx| item(&format!("item-{idx}"), CompletionKind::Command)).collect();
	state.selected_idx = Some(0);
	state.scroll_offset = 0;

	let input = editor.overlay_pane_rect(WindowRole::Input).expect("input pane");
	let hint = editor.utility_overlay_height_hint().expect("utility overlay height hint");
	let visible_rows = editor.completion_visible_rows(CompletionState::MAX_VISIBLE) as u16;

	let target = editor.overlay_completion_menu_target().expect("overlay completion target");
	assert_eq!(target.rect.x, input.x);
	assert_eq!(target.rect.width, input.width);
	assert_eq!(target.rect.y + target.rect.height, input.y);

	let panel_top = input.y.saturating_sub(hint.saturating_sub(1));
	assert!(target.rect.y >= panel_top);
	assert_eq!(target.rect.height, hint.saturating_sub(1).min(visible_rows));
	assert_eq!(target.plan.items.len() as u16, target.rect.height);
}
