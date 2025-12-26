use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::style::{Modifier, Style};
use tome_base::range::CharIdx;
use tome_manifest::Mode;
use tome_theme::blend_colors;

use crate::Editor;

/// Cursor styling configuration for rendering.
pub struct CursorStyles {
	/// Style for the primary (main) cursor.
	pub primary: Style,
	/// Style for secondary (additional) cursors in multi-cursor mode.
	pub secondary: Style,
	/// Base text style.
	pub base: Style,
	/// Selection highlight style.
	pub selection: Style,
}

impl Editor {
	/// Creates cursor styling configuration based on current theme and mode.
	///
	/// # Returns
	/// A [`CursorStyles`] struct containing pre-computed styles for rendering
	/// primary cursors, secondary cursors, selections, and base text.
	pub fn make_cursor_styles(&self) -> CursorStyles {
		let primary_cursor_style = Style::default()
			.bg(self.theme.colors.ui.cursor_bg.into())
			.fg(self.theme.colors.ui.cursor_fg.into())
			.add_modifier(Modifier::BOLD);

		let secondary_cursor_style = {
			let bg = blend_colors(self.theme.colors.ui.cursor_bg, self.theme.colors.ui.bg, 0.4);
			let fg = blend_colors(self.theme.colors.ui.cursor_fg, self.theme.colors.ui.fg, 0.4);
			Style::default()
				.bg(bg.into())
				.fg(fg.into())
				.add_modifier(Modifier::BOLD)
		};

		let base_style =
			Style::default()
				.fg(self.theme.colors.ui.fg.into())
				.bg(self.theme.colors.ui.bg.into());

		let selection_style = Style::default()
			.bg(self.theme.colors.ui.selection_bg.into())
			.fg(self.theme.colors.ui.selection_fg.into());

		CursorStyles {
			primary: primary_cursor_style,
			secondary: secondary_cursor_style,
			base: base_style,
			selection: selection_style,
		}
	}

	/// Checks if the cursor should be visible (blinking state).
	///
	/// In insert mode, cursors blink on a 200ms cycle. In other modes, cursors
	/// are always visible.
	///
	/// # Returns
	/// `true` if the cursor should be rendered, `false` if it should be hidden
	/// due to blink timing.
	pub fn cursor_blink_visible(&self) -> bool {
		let insert_mode = matches!(self.mode(), Mode::Insert);
		if !insert_mode {
			return true;
		}

		let now_ms = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_millis();

		(now_ms / 200).is_multiple_of(2)
	}

	/// Collects all cursor positions from the current selection.
	///
	/// # Returns
	/// A [`HashSet`] containing the character index of each cursor head in the
	/// current selection. Used for efficient lookup during rendering.
	pub fn collect_cursor_heads(&self) -> HashSet<CharIdx> {
		self.buffer()
			.selection
			.ranges()
			.iter()
			.map(|r| r.head)
			.collect()
	}
}
