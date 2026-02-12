//! Layout management for buffer splits.
//!
//! The [`crate::layout::LayoutManager`] manages overlay layers and handles split operations,
//! view navigation, and separator interactions (hover/drag for resizing).
//!
//! # Layer System
//!
//! Layouts are organized in ordered layers (index 0 at bottom):
//! - Layer 0: Base layout (owned by the base window)
//! - Layer 1+: Overlay layers (transparent base, rendered on top)
//!
//! Focus goes to the topmost layer containing views by default.
//!
//! # Modules
//!
//! - `manager` - Core `LayoutManager` struct
//! - `types` - Type definitions (`LayerId`, `SeparatorId`, `SeparatorHit`)
//! - `layers` - Layer management and area computation
//! - `views` - View navigation and lookup
//! - `splits` - Split creation and removal
//! - `separators` - Separator hit detection
//! - `drag` - Drag state and hover animation

mod drag;
mod layers;
pub(crate) mod manager;
mod separators;
mod splits;
mod types;
mod views;

#[cfg(test)]
mod invariants;

pub use manager::LayoutManager;
pub use splits::SplitError;
pub use types::{LayerError, LayerId, SeparatorHit, SeparatorId};

#[cfg(test)]
mod tests {
	use crate::geometry::Rect;

	use super::*;
	use crate::buffer::{Layout, ViewId};

	fn make_doc_area() -> Rect {
		Rect {
			x: 0,
			y: 0,
			width: 80,
			height: 24,
		}
	}

	#[test]
	fn layer_area_base_only() {
		let mgr = LayoutManager::new();
		let doc = make_doc_area();

		let layer0 = mgr.layer_area(LayerId::BASE, doc);
		assert_eq!(layer0, doc, "base layer gets full area");
	}

	#[test]
	fn view_at_position_finds_buffer() {
		let mgr = LayoutManager::new();
		let doc = make_doc_area();
		let base_layout = Layout::text(ViewId(0));

		let hit = mgr.view_at_position(&base_layout, doc, 40, 12);
		assert!(hit.is_some());
		let (view, _) = hit.unwrap();
		assert_eq!(view, ViewId(0), "clicking in base area returns buffer 0");
	}
}
