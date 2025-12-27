//! UI-specific notification types.
//!
//! These types use tome_tui geometry types and are only used in the
//! rendering layer.

use tome_tui::layout::Rect;
use tome_manifest::notifications::{Anchor, AnimationPhase, SlideDirection};

/// Parameters for sliding animations.
///
/// This struct contains all the information needed to calculate
/// slide animation positions.
#[derive(Debug, Clone, Copy)]
pub struct SlideParams {
	/// The full target rectangle for the notification.
	pub full_rect: Rect,
	/// The frame area to render within.
	pub frame_area: Rect,
	/// Animation progress (0.0 to 1.0).
	pub progress: f32,
	/// Current animation phase.
	pub phase: AnimationPhase,
	/// Anchor position.
	pub anchor: Anchor,
	/// Direction to slide from/to.
	pub slide_direction: SlideDirection,
	/// Custom entry position (x, y percentages).
	pub custom_slide_in_start_pos: Option<(f32, f32)>,
	/// Custom exit position (x, y percentages).
	pub custom_slide_out_end_pos: Option<(f32, f32)>,
}
