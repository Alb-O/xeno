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
