use super::render_safe_char;

#[test]
fn maps_escape_to_control_picture() {
	assert_eq!(render_safe_char('\x1b'), '\u{241b}');
}

#[test]
fn maps_cr_to_control_picture() {
	assert_eq!(render_safe_char('\r'), '\u{240d}');
}
