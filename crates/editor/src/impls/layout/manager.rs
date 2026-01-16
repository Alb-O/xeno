//! Layout manager core structure.
//!
//! The [`LayoutManager`] owns stacked layout layers and coordinates all layout operations.

use xeno_tui::layout::Rect;

use crate::buffer::{BufferView, Layout, SplitDirection};
use crate::impls::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};

/// Manages stacked layout layers and separator interactions.
///
/// Layouts are organized in ordered layers. Layer 0 is the base layout
/// owned by the base window; higher layers overlay on top with transparent backgrounds.
pub struct LayoutManager {
	/// Layout layers above the base layout (index 0 reserved for base).
	pub(super) layers: Vec<Option<Layout>>,

	/// Currently hovered separator (for visual feedback during resize).
	pub hovered_separator: Option<(SplitDirection, Rect)>,

	/// Separator the mouse is currently over (regardless of velocity).
	pub separator_under_mouse: Option<(SplitDirection, Rect)>,

	/// Animation state for separator hover fade effects.
	pub separator_hover_animation: Option<SeparatorHoverAnimation>,

	/// Tracks mouse velocity to suppress hover effects during fast movement.
	pub mouse_velocity: MouseVelocityTracker,

	/// Active separator drag state for resizing splits.
	pub dragging_separator: Option<DragState>,

	/// Tracks the view where a text selection drag started.
	pub text_selection_origin: Option<(BufferView, Rect)>,
}

impl Default for LayoutManager {
	fn default() -> Self {
		Self {
			layers: vec![None],
			hovered_separator: None,
			separator_under_mouse: None,
			separator_hover_animation: None,
			mouse_velocity: MouseVelocityTracker::default(),
			dragging_separator: None,
			text_selection_origin: None,
		}
	}
}

impl LayoutManager {
	/// Creates a new layout manager without owning the base layout.
	pub fn new() -> Self {
		Self::default()
	}
}
