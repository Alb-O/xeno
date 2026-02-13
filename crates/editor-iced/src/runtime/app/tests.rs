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
fn format_header_line_formats_snapshot_fields() {
	let header = HeaderSnapshot {
		mode: String::from("INSERT"),
		cursor_line: 3,
		cursor_col: 7,
		buffers: 2,
		ime_preedit: String::from("pre"),
	};
	assert_eq!(format_header_line(&header), "mode=INSERT cursor=3:7 buffers=2 ime_preedit=pre");
}

#[test]
fn viewport_rows_for_document_rows_reserves_statusline_row() {
	assert_eq!(viewport_rows_for_document_rows(0), 1);
	assert_eq!(viewport_rows_for_document_rows(5), 6);
}

#[test]
fn viewport_grid_from_document_size_keeps_columns_and_adds_statusline_row() {
	let metrics = super::super::CellMetrics::from_env();
	let (expected_cols, expected_document_rows) = metrics.to_grid(160.0, 80.0);
	let (cols, rows) = viewport_grid_from_document_size(metrics, iced::Size::new(160.0, 80.0));

	assert_eq!(cols, expected_cols);
	assert_eq!(rows, viewport_rows_for_document_rows(expected_document_rows));
}

#[test]
fn font_size_for_cell_metrics_clamps_to_cell_height() {
	let metrics = super::super::CellMetrics::from_env();
	let font_size = font_size_for_cell_metrics(metrics);

	assert!(font_size >= 1.0);
	assert!(font_size <= metrics.height_px());
}

#[test]
fn parse_coordinate_scale_validates_input() {
	assert_eq!(parse_coordinate_scale(Some("1.25")), Some(1.25));
	assert_eq!(parse_coordinate_scale(Some("0")), None);
	assert_eq!(parse_coordinate_scale(Some("-1")), None);
	assert_eq!(parse_coordinate_scale(Some("abc")), None);
	assert_eq!(parse_coordinate_scale(None), None);
}

#[test]
fn coordinate_scale_normalizes_point_and_size() {
	let scale = CoordinateScale { x: 2.0, y: 4.0 };
	assert_eq!(scale.normalize_point(iced::Point::new(20.0, 40.0)), iced::Point::new(10.0, 10.0));
	assert_eq!(scale.normalize_size(iced::Size::new(200.0, 80.0)), iced::Size::new(100.0, 20.0));
}
