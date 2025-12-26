//! Split buffer trait for content that can be rendered in dock panels.
//!
//! A `SplitBuffer` represents any content that can occupy a split region of the
//! editor window. Examples include embedded terminals, log viewers, file trees,
//! and preview panes.
//!
//! This trait defines the core interface that split buffers must implement,
//! decoupled from specific UI framework types. The rendering layer (`tome-api`)
//! wraps these in `Panel` implementations.

use std::time::Duration;

use tome_base::Position;

/// Keyboard input for split buffers.
///
/// A simplified key event that split buffers can handle without depending on
/// terminal-specific key types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitKey {
	/// The key code.
	pub code: SplitKeyCode,
	/// Modifier keys held during the key press.
	pub modifiers: SplitModifiers,
}

impl SplitKey {
	/// Creates a new key event.
	pub const fn new(code: SplitKeyCode, modifiers: SplitModifiers) -> Self {
		Self { code, modifiers }
	}

	/// Creates a key event with no modifiers.
	pub const fn plain(code: SplitKeyCode) -> Self {
		Self::new(code, SplitModifiers::NONE)
	}

	/// Creates a character key event.
	pub const fn char(c: char) -> Self {
		Self::plain(SplitKeyCode::Char(c))
	}

	/// Creates a character key event with Ctrl held.
	pub const fn ctrl(c: char) -> Self {
		Self::new(SplitKeyCode::Char(c), SplitModifiers::CTRL)
	}
}

/// Key codes for split buffer input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitKeyCode {
	Char(char),
	Enter,
	Escape,
	Backspace,
	Tab,
	Up,
	Down,
	Left,
	Right,
	Home,
	End,
	PageUp,
	PageDown,
	Delete,
	Insert,
	F(u8),
}

/// Modifier key flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SplitModifiers(u8);

impl SplitModifiers {
	pub const NONE: Self = Self(0);
	pub const CTRL: Self = Self(1);
	pub const ALT: Self = Self(2);
	pub const SHIFT: Self = Self(4);

	pub const fn contains(self, other: Self) -> bool {
		(self.0 & other.0) == other.0
	}

	pub const fn union(self, other: Self) -> Self {
		Self(self.0 | other.0)
	}
}

/// Mouse input for split buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMouse {
	/// Position relative to the buffer's top-left corner.
	pub position: Position,
	/// The mouse action.
	pub action: SplitMouseAction,
}

/// Mouse action types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitMouseAction {
	Press(SplitMouseButton),
	Release(SplitMouseButton),
	Drag(SplitMouseButton),
	ScrollUp,
	ScrollDown,
}

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitMouseButton {
	Left,
	Right,
	Middle,
}

/// Result of handling an input event.
#[derive(Debug, Clone, Default)]
pub struct SplitEventResult {
	/// Whether the event was consumed by the buffer.
	pub consumed: bool,
	/// Whether the buffer content changed and needs redraw.
	pub needs_redraw: bool,
	/// Whether to release focus from this buffer.
	pub release_focus: bool,
	/// Whether the buffer wants to close itself.
	pub wants_close: bool,
}

impl SplitEventResult {
	/// Event was consumed, redraw needed.
	pub fn consumed() -> Self {
		Self {
			consumed: true,
			needs_redraw: true,
			..Default::default()
		}
	}

	/// Event was not consumed.
	pub fn ignored() -> Self {
		Self::default()
	}

	/// Event was consumed but no redraw needed.
	pub fn consumed_quiet() -> Self {
		Self {
			consumed: true,
			..Default::default()
		}
	}

	/// Builder: request focus release.
	pub fn with_release_focus(mut self) -> Self {
		self.release_focus = true;
		self
	}

	/// Builder: request buffer close.
	pub fn with_close(mut self) -> Self {
		self.wants_close = true;
		self
	}
}

/// Cursor style hint for the hosting UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitCursorStyle {
	#[default]
	Hidden,
	Default,
	Block,
	Bar,
	Underline,
}

/// Cursor information returned by split buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitCursor {
	/// Row position (0-indexed, relative to buffer area).
	pub row: u16,
	/// Column position (0-indexed, relative to buffer area).
	pub col: u16,
	/// Cursor style.
	pub style: SplitCursorStyle,
}

/// Size in terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SplitSize {
	pub width: u16,
	pub height: u16,
}

impl SplitSize {
	pub const fn new(width: u16, height: u16) -> Self {
		Self { width, height }
	}

	pub const fn is_empty(&self) -> bool {
		self.width == 0 || self.height == 0
	}
}

/// Trait for content that can be displayed in a split panel.
///
/// Implementors provide their own content management, input handling, and
/// rendering logic. The UI layer (`tome-api`) wraps these in `Panel`
/// implementations to integrate with the dock system.
///
/// # Lifecycle
///
/// 1. `on_open()` - Called when the panel opens
/// 2. `resize()` - Called with initial size and on terminal resize
/// 3. `tick()` - Called periodically for async content updates
/// 4. `handle_key()`/`handle_mouse()`/`handle_paste()` - Input routing
/// 5. `on_close()` - Called when the panel closes
///
/// # Rendering
///
/// Split buffers don't render directly. Instead, they expose content via
/// the `cells()` iterator. The UI layer consumes this to render into a
/// ratatui buffer.
pub trait SplitBuffer: Send {
	/// Unique identifier for this buffer type.
	fn id(&self) -> &str;

	/// Called when the buffer is opened/shown.
	fn on_open(&mut self) {}

	/// Called when the buffer is closed/hidden.
	fn on_close(&mut self) {}

	/// Called when focus changes to/from this buffer.
	fn on_focus_changed(&mut self, _focused: bool) {}

	/// Notify the buffer of a size change.
	fn resize(&mut self, size: SplitSize);

	/// Periodic tick for async updates (terminal output, etc).
	///
	/// Returns a result indicating whether content changed, focus should release,
	/// or the buffer wants to close.
	fn tick(&mut self, _delta: Duration) -> SplitEventResult {
		SplitEventResult::ignored()
	}

	/// Handle a key event.
	fn handle_key(&mut self, key: SplitKey) -> SplitEventResult;

	/// Handle mouse input.
	fn handle_mouse(&mut self, _mouse: SplitMouse) -> SplitEventResult {
		SplitEventResult::ignored()
	}

	/// Handle pasted text.
	fn handle_paste(&mut self, _text: &str) -> SplitEventResult {
		SplitEventResult::ignored()
	}

	/// Returns the current size of the buffer content.
	fn size(&self) -> SplitSize;

	/// Returns cursor information if visible.
	fn cursor(&self) -> Option<SplitCursor>;

	/// Iterate over cells for rendering.
	///
	/// The closure receives `(row, col, cell)` for each cell that has content.
	/// Cells not yielded are assumed to be empty with default styling.
	fn for_each_cell<F>(&self, f: F)
	where
		F: FnMut(u16, u16, &SplitCell);
}

/// A single cell in a split buffer.
#[derive(Debug, Clone, Default)]
pub struct SplitCell {
	/// The character(s) to display. Empty string means space.
	pub symbol: String,
	/// Foreground color (None = default).
	pub fg: Option<SplitColor>,
	/// Background color (None = default).
	pub bg: Option<SplitColor>,
	/// Text attributes.
	pub attrs: SplitAttrs,
	/// Whether this cell is the continuation of a wide character.
	pub wide_continuation: bool,
}

impl SplitCell {
	pub fn new(symbol: impl Into<String>) -> Self {
		Self {
			symbol: symbol.into(),
			..Default::default()
		}
	}

	pub fn with_fg(mut self, color: SplitColor) -> Self {
		self.fg = Some(color);
		self
	}

	pub fn with_bg(mut self, color: SplitColor) -> Self {
		self.bg = Some(color);
		self
	}

	pub fn with_attrs(mut self, attrs: SplitAttrs) -> Self {
		self.attrs = attrs;
		self
	}
}

/// Color for split buffer cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitColor {
	/// ANSI 256-color palette index.
	Indexed(u8),
	/// True color RGB.
	Rgb(u8, u8, u8),
}

/// Text attributes for split buffer cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SplitAttrs(u8);

impl SplitAttrs {
	pub const NONE: Self = Self(0);
	pub const BOLD: Self = Self(1);
	pub const ITALIC: Self = Self(2);
	pub const UNDERLINE: Self = Self(4);
	pub const INVERSE: Self = Self(8);

	pub const fn contains(self, other: Self) -> bool {
		(self.0 & other.0) == other.0
	}

	pub const fn union(self, other: Self) -> Self {
		Self(self.0 | other.0)
	}
}

/// Where the split buffer prefers to be docked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitDockPreference {
	#[default]
	Bottom,
	Top,
	Left,
	Right,
}
