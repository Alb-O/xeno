//! Layout type definitions.

use super::super::BufferId;

/// Path to a split in the layout tree.
///
/// Each element indicates which branch to take: `false` for first child,
/// `true` for second child. An empty path refers to the root split.
///
/// This provides a stable way to identify splits that doesn't change
/// when ratios are adjusted during resize operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitPath(pub Vec<bool>);

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
	/// Horizontal split (buffers side by side).
	Horizontal,
	/// Vertical split (buffers stacked).
	Vertical,
}

/// Unique identifier for a terminal buffer.
///
/// Terminal IDs are assigned sequentially starting from 1 when terminals
/// are created via [`Editor::split_horizontal_terminal`] or
/// [`Editor::split_vertical_terminal`].
///
/// [`Editor::split_horizontal_terminal`]: crate::Editor::split_horizontal_terminal
/// [`Editor::split_vertical_terminal`]: crate::Editor::split_vertical_terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub u64);

/// A view in the layout - either a text buffer or a terminal.
///
/// This enum enables the layout system to manage heterogeneous content types
/// in splits. The editor tracks the focused view via this type, allowing
/// seamless navigation between text editing and terminal sessions.
///
/// # Focus Handling
///
/// When a terminal is focused, text-editing operations are unavailable.
/// Use [`Editor::is_text_focused`] or [`Editor::is_terminal_focused`] to
/// check focus type before operations.
///
/// [`Editor::is_text_focused`]: crate::Editor::is_text_focused
/// [`Editor::is_terminal_focused`]: crate::Editor::is_terminal_focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferView {
	/// A text buffer for document editing.
	Text(BufferId),
	/// An embedded terminal emulator.
	Terminal(TerminalId),
}

impl BufferView {
	/// Returns the text buffer ID if this is a text view.
	pub fn as_text(&self) -> Option<BufferId> {
		match self {
			BufferView::Text(id) => Some(*id),
			BufferView::Terminal(_) => None,
		}
	}

	/// Returns the terminal ID if this is a terminal view.
	pub fn as_terminal(&self) -> Option<TerminalId> {
		match self {
			BufferView::Text(_) => None,
			BufferView::Terminal(id) => Some(*id),
		}
	}

	/// Returns true if this is a text buffer view.
	pub fn is_text(&self) -> bool {
		matches!(self, BufferView::Text(_))
	}

	/// Returns true if this is a terminal view.
	pub fn is_terminal(&self) -> bool {
		matches!(self, BufferView::Terminal(_))
	}
}

impl From<BufferId> for BufferView {
	fn from(id: BufferId) -> Self {
		BufferView::Text(id)
	}
}

impl From<TerminalId> for BufferView {
	fn from(id: TerminalId) -> Self {
		BufferView::Terminal(id)
	}
}
