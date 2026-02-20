use super::*;

/// Manages stacked layout layers and separator interactions.
///
/// Layouts are organized in ordered layers. Layer 0 is the base layout
/// owned by the base window; higher layers overlay on top with
/// transparent backgrounds.
///
/// Overlay layers use generational tracking to prevent stale references.
pub struct LayoutManager {
	/// Layout layers above the base layout (index 0 reserved for base).
	/// Uses generational slots to prevent stale references.
	pub(in crate::layout) layers: Vec<LayerSlot>,

	/// Revision counter incremented on structural layout changes.
	///
	/// Used to detect stale drag state when the layout changes mid-drag
	/// (e.g., a view is closed while dragging a separator).
	structure_revision: u64,

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
	pub text_selection_origin: Option<(ViewId, Rect)>,
}

impl Default for LayoutManager {
	fn default() -> Self {
		Self {
			layers: vec![LayerSlot::empty()],
			structure_revision: 0,
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
	/// Creates a new `LayoutManager` without owning the base layout.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns the current structural layout revision.
	///
	/// Structural changes (splits, removals) increment this value.
	pub fn structure_revision(&self) -> u64 {
		self.structure_revision
	}

	/// Increments the structural layout revision counter.
	///
	/// Call this after any structural change to the layout.
	pub(in crate::layout) fn bump_structure_revision(&mut self) {
		self.structure_revision = self.structure_revision.wrapping_add(1);
	}
}
