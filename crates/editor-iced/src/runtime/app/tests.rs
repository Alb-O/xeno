use super::*;

#[test]
fn is_paste_shortcut_matches_command_v() {
	let key = keyboard::Key::Character("v".into());
	let physical = keyboard::key::Physical::Code(keyboard::key::Code::KeyV);
	assert!(is_paste_shortcut(&key, &key, physical, keyboard::Modifiers::COMMAND));
}

#[test]
fn is_paste_shortcut_matches_named_paste_key() {
	let key = keyboard::Key::Named(keyboard::key::Named::Paste);
	let physical = keyboard::key::Physical::Code(keyboard::key::Code::Paste);
	assert!(is_paste_shortcut(&key, &key, physical, keyboard::Modifiers::default()));
}

#[test]
fn map_ui_color_maps_reset_and_rgb() {
	assert_eq!(map_ui_color(UiColor::Reset), None);
	assert_eq!(map_ui_color(UiColor::Rgb(1, 2, 3)), Some(Color::from_rgb8(1, 2, 3)));
}

#[test]
fn style_fg_to_iced_reads_foreground_color() {
	let style = UiStyle::default().fg(UiColor::LightBlue);
	assert_eq!(style_fg_to_iced(style), Some(Color::from_rgb8(0x00, 0x00, 0xFF)));
}

#[test]
fn style_bg_to_iced_reads_background_color() {
	let style = UiStyle::default().bg(UiColor::LightYellow);
	assert_eq!(style_bg_to_iced(style), Some(Color::from_rgb8(0xFF, 0xFF, 0x00)));
}

#[test]
fn map_ui_color_maps_indexed_palette() {
	assert_eq!(map_ui_color(UiColor::Indexed(16)), Some(Color::from_rgb8(0, 0, 0)));
	assert_eq!(map_ui_color(UiColor::Indexed(21)), Some(Color::from_rgb8(0, 0, 255)));
	assert_eq!(map_ui_color(UiColor::Indexed(231)), Some(Color::from_rgb8(255, 255, 255)));
	assert_eq!(map_ui_color(UiColor::Indexed(232)), Some(Color::from_rgb8(8, 8, 8)));
	assert_eq!(map_ui_color(UiColor::Indexed(255)), Some(Color::from_rgb8(238, 238, 238)));
}

#[test]
fn parse_inspector_width_validates_bounds_and_fallback() {
	assert_eq!(parse_inspector_width(None), DEFAULT_INSPECTOR_WIDTH_PX);
	assert_eq!(parse_inspector_width(Some("500")), 500.0);
	assert_eq!(parse_inspector_width(Some("159.0")), DEFAULT_INSPECTOR_WIDTH_PX);
	assert_eq!(parse_inspector_width(Some("abc")), DEFAULT_INSPECTOR_WIDTH_PX);
}

#[test]
fn parse_show_inspector_understands_common_false_values() {
	assert!(parse_show_inspector(None));
	assert!(parse_show_inspector(Some("1")));
	assert!(parse_show_inspector(Some("true")));
	assert!(!parse_show_inspector(Some("0")));
	assert!(!parse_show_inspector(Some("false")));
	assert!(!parse_show_inspector(Some("No")));
	assert!(!parse_show_inspector(Some("off")));
}

#[test]
fn completion_preview_marks_selected_rows() {
	let plan = CompletionRenderPlan {
		max_label_width: 8,
		target_row_width: 40,
		show_kind: false,
		show_right: false,
		items: vec![
			xeno_editor::completion::CompletionRenderItem {
				label: String::from("alpha"),
				kind: xeno_editor::completion::CompletionKind::Command,
				right: None,
				match_indices: None,
				selected: false,
				command_alias_match: false,
			},
			xeno_editor::completion::CompletionRenderItem {
				label: String::from("beta"),
				kind: xeno_editor::completion::CompletionKind::Command,
				right: None,
				match_indices: None,
				selected: true,
				command_alias_match: false,
			},
		],
	};

	let row = completion_row_label(&plan, &plan.items[1]);
	assert_eq!(row, "> beta");
}

#[test]
fn snippet_row_label_prefixes_selected_rows() {
	let selected = SnippetChoiceRenderItem {
		option: String::from("choice-a"),
		selected: true,
	};
	let normal = SnippetChoiceRenderItem {
		option: String::from("choice-b"),
		selected: false,
	};

	assert_eq!(snippet_row_label(&selected), "> choice-a");
	assert_eq!(snippet_row_label(&normal), "  choice-b");
}
