use super::*;

fn test_context(is_cursor_line: bool, diff_bg: Option<Color>) -> LineStyleContext {
	LineStyleContext {
		base_bg: Color::Rgb(30, 30, 30),
		diff_bg,
		mode_color: Color::Rgb(100, 150, 200),
		is_cursor_line,
		cursorline_enabled: true,
		cursor_line: 0,
		is_nontext: false,
	}
}

#[test]
fn fill_bg_no_cursor_no_diff() {
	let ctx = test_context(false, None);
	assert!(ctx.fill_bg().is_none());
}

#[test]
fn fill_bg_cursor_no_diff() {
	let ctx = test_context(true, None);
	assert!(ctx.fill_bg().is_some());
}

#[test]
fn fill_bg_no_cursor_with_diff() {
	let diff = Color::Rgb(50, 80, 50);
	let ctx = test_context(false, Some(diff));
	assert_eq!(ctx.fill_bg(), Some(diff));
}

#[test]
fn fill_bg_cursor_with_diff() {
	let diff = Color::Rgb(50, 80, 50);
	let ctx = test_context(true, Some(diff));
	let result = ctx.fill_bg().unwrap();
	assert_ne!(result, diff);
}

#[test]
fn cursorline_disabled() {
	let mut ctx = test_context(true, None);
	ctx.cursorline_enabled = false;
	assert!(ctx.fill_bg().is_none());
}
