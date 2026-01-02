//! Layout type definitions.
//!
//! Core types used across the layout system.

use evildoer_tui::layout::Rect;

use crate::buffer::{SplitDirection, SplitPath};

/// Layer index for layout operations.
pub type LayerIndex = usize;

/// Identifies which separator is being interacted with.
#[derive(Debug, Clone, PartialEq)]
pub enum SeparatorId {
	/// A separator within a layer's split tree.
	Split {
		/// Path identifying the split in the tree.
		path: SplitPath,
		/// Index of the layer containing this split.
		layer: LayerIndex,
	},
	/// The boundary between layer 0 and layer 1 (bottom dock boundary).
	LayerBoundary,
	/// The boundary between layer 0 and layer 2 (side dock boundary).
	SideBoundary,
}

/// Information about a separator found at a screen position.
#[derive(Debug, Clone)]
pub struct SeparatorHit {
	/// The separator that was hit.
	pub id: SeparatorId,
	/// Whether this separator divides horizontally or vertically.
	pub direction: SplitDirection,
	/// Screen bounds of the separator.
	pub rect: Rect,
}
