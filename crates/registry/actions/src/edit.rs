//! Edit operations and scroll/movement types.
//!
//! These types represent buffer modifications and movement operations
//! that are triggered by editing commands like `d`, `c`, `y`, etc.

/// Edit operation to apply to buffer content.
///
/// These operations modify text and are typically triggered by editing
/// commands like `d` (delete), `c` (change), `y` (yank), etc.
#[derive(Debug, Clone)]
pub enum EditAction {
	/// Delete text (optionally yanking to register first).
	Delete {
		/// Whether to yank before deleting.
		yank: bool,
	},
	/// Change text (delete and enter insert mode).
	Change {
		/// Whether to yank before changing.
		yank: bool,
	},
	/// Yank (copy) text to register.
	Yank,
	/// Paste from register.
	Paste {
		/// Whether to paste before cursor (vs after).
		before: bool,
	},
	/// Paste to all selections from register.
	PasteAll {
		/// Whether to paste before cursor (vs after).
		before: bool,
	},
	/// Replace selected text with a single character.
	ReplaceWithChar {
		/// The replacement character.
		ch: char,
	},
	/// Undo the last change.
	Undo,
	/// Redo the last undone change.
	Redo,
	/// Increase indentation of selected lines.
	Indent,
	/// Decrease indentation of selected lines.
	Deindent,
	/// Convert selected text to lowercase.
	ToLowerCase,
	/// Convert selected text to uppercase.
	ToUpperCase,
	/// Swap case of selected text.
	SwapCase,
	/// Join selected lines into one.
	JoinLines,
	/// Delete character before cursor (backspace).
	DeleteBack,
	/// Open new line below and enter insert mode.
	OpenBelow,
	/// Open new line above and enter insert mode.
	OpenAbove,
	/// Move selection visually (up/down wrapped lines).
	MoveVisual {
		/// Direction to move.
		direction: VisualDirection,
		/// Number of visual lines to move.
		count: usize,
		/// Whether to extend selection rather than move.
		extend: bool,
	},
	/// Scroll the viewport.
	Scroll {
		/// Direction to scroll.
		direction: ScrollDir,
		/// How much to scroll.
		amount: ScrollAmount,
		/// Whether to extend selection while scrolling.
		extend: bool,
	},
	/// Add a blank line below cursor line.
	AddLineBelow,
	/// Add a blank line above cursor line.
	AddLineAbove,
}

/// Direction for visual line movement operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualDirection {
	/// Move up (toward beginning of buffer).
	Up,
	/// Move down (toward end of buffer).
	Down,
}

/// Direction for scrolling operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDir {
	/// Scroll up (view moves toward beginning).
	Up,
	/// Scroll down (view moves toward end).
	Down,
}

/// Amount to scroll (lines or page fraction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAmount {
	/// Scroll by a specific number of lines.
	Line(usize),
	/// Scroll by half a page.
	HalfPage,
	/// Scroll by a full page.
	FullPage,
}
