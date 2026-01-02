//! Layout type definitions.

use evildoer_registry::panels::PanelId;

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

/// A view in the layout - either a text buffer or a panel.
///
/// This enum enables the layout system to manage heterogeneous content types
/// in splits. The editor tracks the focused view via this type, allowing
/// seamless navigation between text editing and panel content.
///
/// # Focus Handling
///
/// When a panel is focused, text-editing operations are unavailable.
/// Use [`Editor::is_text_focused`] to check focus type before operations.
///
/// [`Editor::is_text_focused`]: crate::Editor::is_text_focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferView {
	/// A text buffer for document editing.
	Text(BufferId),
	/// A panel (terminal, debug, file tree, etc.).
	Panel(PanelId),
}

impl BufferView {
	/// Returns the text buffer ID if this is a text view.
	pub fn as_text(&self) -> Option<BufferId> {
		match self {
			BufferView::Text(id) => Some(*id),
			BufferView::Panel(_) => None,
		}
	}

	/// Returns the panel ID if this is a panel view.
	pub fn as_panel(&self) -> Option<PanelId> {
		match self {
			BufferView::Panel(id) => Some(*id),
			BufferView::Text(_) => None,
		}
	}

	/// Returns true if this is a text buffer view.
	pub fn is_text(&self) -> bool {
		matches!(self, BufferView::Text(_))
	}

	/// Returns true if this is a panel view.
	pub fn is_panel(&self) -> bool {
		matches!(self, BufferView::Panel(_))
	}

	/// Returns the visual priority of this view type.
	///
	/// Higher values indicate lighter backgrounds. Separators use the background
	/// color of the adjacent view with the highest priority.
	pub fn visual_priority(&self) -> u8 {
		match self {
			BufferView::Text(_) => 0,
			BufferView::Panel(_) => 1,
		}
	}
}

impl From<BufferId> for BufferView {
	fn from(id: BufferId) -> Self {
		BufferView::Text(id)
	}
}

impl From<PanelId> for BufferView {
	fn from(id: PanelId) -> Self {
		BufferView::Panel(id)
	}
}
