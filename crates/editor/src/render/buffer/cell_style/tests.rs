use xeno_primitives::Color;

use super::*;

fn test_line_ctx() -> LineStyleContext {
	LineStyleContext {
		base_bg: Color::Rgb(30, 30, 30),
		diff_bg: None,
		mode_color: Color::Rgb(100, 150, 200),
		is_cursor_line: false,
		cursorline_enabled: true,
		cursor_line: 0,
		is_nontext: false,
	}
}

fn test_cursor_styles() -> CursorStyleSet {
	CursorStyleSet {
		primary: Style::default().bg(Color::Rgb(100, 150, 200)),
		secondary: Style::default().bg(Color::Rgb(70, 100, 140)),
		unfocused: Style::default().bg(Color::Rgb(50, 50, 50)),
	}
}

#[test]
fn basic_resolution() {
	let line_ctx = test_line_ctx();
	let cursor_styles = test_cursor_styles();
	let input = CellStyleInput {
		line_ctx: &line_ctx,
		syntax_style: None,
		in_selection: false,
		is_primary_cursor: false,
		is_focused: true,
		cursor_styles: &cursor_styles,
		base_style: Style::default().fg(Color::White),
	};

	let result = resolve_cell_style(input);
	assert_eq!(result.non_cursor.fg, Some(Color::White));
}

#[test]
fn selection_has_background() {
	let line_ctx = test_line_ctx();
	let cursor_styles = test_cursor_styles();
	let input = CellStyleInput {
		line_ctx: &line_ctx,
		syntax_style: Some(Style::default().fg(Color::Yellow)),
		in_selection: true,
		is_primary_cursor: false,
		is_focused: true,
		cursor_styles: &cursor_styles,
		base_style: Style::default(),
	};

	let result = resolve_cell_style(input);
	assert!(result.non_cursor.bg.is_some());
}
