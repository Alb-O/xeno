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
	Split { path: SplitPath, layer: LayerIndex },
	/// The boundary between layer 0 and layer 1 (bottom dock boundary).
	LayerBoundary,
	/// The boundary between layer 0 and layer 2 (side dock boundary).
	SideBoundary,
}

/// Information about a separator found at a screen position.
#[derive(Debug, Clone)]
pub struct SeparatorHit {
	pub id: SeparatorId,
	pub direction: SplitDirection,
	pub rect: Rect,
}
