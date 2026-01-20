//! Per-character style resolution for buffer rendering.
//!
//! Handles the style cascade: cursor > selection > cursorline > syntax > base.

use xeno_tui::style::Style;

use super::style_layers::{LineStyleContext, blend};

/// Input for resolving a cell's style.
#[derive(Debug, Clone, Copy)]
pub struct CellStyleInput<'a> {
	/// Line-level style context.
	pub line_ctx: &'a LineStyleContext,
	/// Syntax highlighting style for this character.
	pub syntax_style: Option<Style>,
	/// Whether this character is in a selection range.
	pub in_selection: bool,
	/// Whether this is the primary cursor.
	pub is_primary_cursor: bool,
	/// Whether the buffer is focused.
	pub is_focused: bool,
	/// Cursor styles from theme/mode.
	pub cursor_styles: &'a CursorStyleSet,
	/// Base text style (fallback).
	pub base_style: Style,
}

/// Set of cursor styles for different cursor states.
#[derive(Debug, Clone, Copy)]
pub struct CursorStyleSet {
	/// Style for the primary (main) cursor.
	pub primary: Style,
	/// Style for secondary cursors in multi-cursor mode.
	pub secondary: Style,
	/// Style for cursors in unfocused buffers.
	pub unfocused: Style,
}

/// Resolves the style for a character cell.
///
/// Applies the style cascade in order:
/// 1. Cursor (if cursor position and block cursor enabled)
/// 2. Selection (blends bg + mode + syntax tint)
/// 3. Cursorline (blends into existing bg)
/// 4. Syntax highlighting
/// 5. Base style
///
/// Returns the computed style and the non-cursor style (for cursor rendering
/// where we need both).
pub fn resolve_cell_style(input: CellStyleInput<'_>) -> ResolvedCellStyle {
	let cursor_style = if !input.is_focused {
		input.cursor_styles.unfocused
	} else if input.is_primary_cursor {
		input.cursor_styles.primary
	} else {
		input.cursor_styles.secondary
	};

	let non_cursor_style = resolve_non_cursor_style(input);

	ResolvedCellStyle {
		cursor: cursor_style,
		non_cursor: non_cursor_style,
	}
}

/// Resolved styles for a cell.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedCellStyle {
	/// Style to use when rendering as a cursor.
	pub cursor: Style,
	/// Style to use when not rendering as a cursor.
	pub non_cursor: Style,
}

impl ResolvedCellStyle {
	/// Returns the appropriate style based on whether to show cursor.
	#[allow(dead_code, reason = "utility method for callers")]
	pub fn select(self, show_cursor: bool) -> Style {
		if show_cursor {
			self.cursor
		} else {
			self.non_cursor
		}
	}
}

/// Resolves the non-cursor style for a character.
fn resolve_non_cursor_style(input: CellStyleInput<'_>) -> Style {
	let base = input.syntax_style.unwrap_or(input.base_style);

	if input.in_selection {
		resolve_selection_style(input, base)
	} else if input.line_ctx.should_highlight_cursorline() {
		resolve_cursorline_style(input, base)
	} else {
		base
	}
}

/// Computes selection highlight style.
fn resolve_selection_style(input: CellStyleInput<'_>, base: Style) -> Style {
	let syntax_fg = base.fg.or(input.base_style.fg).unwrap_or(input.line_ctx.base_bg);
	let selection_bg = input
		.line_ctx
		.base_bg
		.blend(input.line_ctx.mode_color, blend::SELECTION_MODE_ALPHA)
		.blend(syntax_fg, blend::SELECTION_SYNTAX_ALPHA)
		.ensure_min_contrast(input.line_ctx.base_bg, blend::SELECTION_MIN_CONTRAST);

	Style::default()
		.bg(selection_bg)
		.fg(syntax_fg)
		.add_modifier(base.add_modifier)
}

/// Computes cursorline style, blending into existing syntax bg.
fn resolve_cursorline_style(input: CellStyleInput<'_>, base: Style) -> Style {
	let blended_bg = base
		.bg
		.map(|bg| bg.blend(input.line_ctx.mode_color, blend::CURSORLINE_ALPHA))
		.unwrap_or_else(|| input.line_ctx.cursorline_bg());

	base.bg(blended_bg)
}

#[cfg(test)]
mod tests {
	use xeno_tui::style::Color;

	use super::*;

	fn test_line_ctx() -> LineStyleContext {
		LineStyleContext {
			base_bg: Color::Rgb(30, 30, 30),
			diff_bg: None,
			mode_color: Color::Rgb(100, 150, 200),
			is_cursor_line: false,
			cursorline_enabled: true,
			cursor_line: 0,
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
}
