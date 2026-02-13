use super::*;

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
fn background_style_uses_color_mapping_with_black_fallback() {
	let mapped = background_style(UiColor::Blue);
	assert_eq!(mapped.background, Some(iced::Background::Color(Color::from_rgb8(0x00, 0x00, 0x80))));

	let fallback = background_style(UiColor::Reset);
	assert_eq!(fallback.background, Some(iced::Background::Color(Color::BLACK)));
}

#[test]
fn format_palette_completion_row_includes_kind_and_right_columns() {
	let plan = CompletionRenderPlan {
		items: Vec::new(),
		max_label_width: 8,
		target_row_width: 20,
		show_kind: true,
		show_right: true,
	};
	let item = CompletionRenderItem {
		label: String::from("write"),
		kind: CompletionKind::Command,
		right: Some(String::from("w")),
		match_indices: None,
		selected: false,
		command_alias_match: false,
	};

	let row = format_palette_completion_row(&plan, &item);
	assert!(row.contains("write"));
	assert!(row.contains("[Cmd]"));
	assert!(row.contains(" w"));
}
