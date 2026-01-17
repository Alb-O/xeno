//! Layout type definitions.

pub use xeno_primitives::ViewId;

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
