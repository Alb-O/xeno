//! Layout manager core structure.
//!
//! The [`LayoutManager`] owns stacked layout layers and coordinates all layout operations.

use xeno_tui::layout::Rect;

use crate::buffer::{BufferId, BufferView, Layout, SplitDirection};
use crate::editor::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};

/// Manages stacked layout layers and separator interactions.
///
/// Layouts are organized in ordered layers. Layer 0 is the base (opaque),
/// higher layers overlay on top with transparent backgrounds.
pub struct LayoutManager {
	/// Layout layers, index 0 is base (bottom), higher indices overlay on top.
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

impl LayoutManager {
	/// Creates a new layout manager with a single text buffer on the base layer.
	pub fn new(buffer_id: BufferId) -> Self {
		Self {
			layers: vec![Some(Layout::text(buffer_id))],
			hovered_separator: None,
			separator_under_mouse: None,
			separator_hover_animation: None,
			mouse_velocity: MouseVelocityTracker::default(),
			dragging_separator: None,
			text_selection_origin: None,
		}
	}
}
