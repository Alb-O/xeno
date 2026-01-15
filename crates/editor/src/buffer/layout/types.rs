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

/// Type alias for buffer views in the layout.
///
/// All views in the layout are now text buffers. This type alias maintains
/// API compatibility while the layout system only supports text buffers.
pub type BufferView = BufferId;
