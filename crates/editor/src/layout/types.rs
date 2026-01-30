//! Layout type definitions.
//!
//! Core types used across the layout system.

use xeno_tui::layout::Rect;

use crate::buffer::{SplitDirection, SplitPath};

/// A generational layer identifier for safe layer references.
///
/// Unlike a raw index, `LayerId` includes a generation counter that
/// increments when a layer is cleared. This prevents stale references
/// from accessing the wrong layer after compaction or reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId {
	/// The slot index in the layers vector.
	pub(crate) idx: u16,
	/// Generation counter for detecting stale references.
	pub(crate) generation: u16,
}

impl LayerId {
	/// The base layer identifier.
	///
	/// The base layer is always valid; its generation is always 0.
	pub const BASE: LayerId = LayerId {
		idx: 0,
		generation: 0,
	};

	/// Creates a new `LayerId`.
	pub(crate) fn new(idx: u16, generation: u16) -> Self {
		Self { idx, generation }
	}

	/// Returns `true` if this is the base layer.
	pub fn is_base(self) -> bool {
		self.idx == 0
	}

	/// Returns the layer index.
	///
	/// For overlay layers, this is the index into the overlay storage.
	/// Returns 0 for the base layer.
	pub fn index(&self) -> usize {
		self.idx as usize
	}
}

/// A slot in the layer storage with generational tracking.
pub(crate) struct LayerSlot {
	/// Generation counter for this slot.
	/// Incremented each time the layer is cleared.
	pub generation: u16,
	/// The layout stored in this slot, if any.
	pub layout: Option<crate::buffer::Layout>,
}

impl LayerSlot {
	/// Creates a new empty layer slot starting at generation 0.
	pub fn empty() -> Self {
		Self {
			generation: 0,
			layout: None,
		}
	}
}

/// Identifies which separator is being interacted with.
#[derive(Debug, Clone, PartialEq)]
pub enum SeparatorId {
	/// A separator within a layer's split tree.
	Split {
		/// Path identifying the split in the tree.
		path: SplitPath,
		/// Generational layer ID for safe referencing.
		layer: LayerId,
	},
}

/// Errors that can occur when validating layer references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerError {
	/// The layer ID has an expired generation (layer was cleared or reused).
	StaleLayer,
	/// The layer slot is empty.
	EmptyLayer,
	/// The layer index is out of bounds.
	InvalidIndex,
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
