use tome_manifest::notifications::AnimationPhase;
use tome_tui::animation::Animatable;
use tome_tui::prelude::*;

/// Minimum width for expand/collapse animation to remain visible.
const MIN_WIDTH: u16 = 3;
/// Minimum height for expand/collapse animation to remain visible.
const MIN_HEIGHT: u16 = 3;

/// Calculates the rectangle for expand/collapse animation at a given progress.
///
/// This function animates a notification expanding from a small centered rectangle
/// to its full size, or collapsing from full size to a small rectangle.
///
/// # Parameters
/// - `full_rect`: The target rectangle when fully expanded
/// - `_frame_area`: The containing frame area (reserved for future use)
/// - `phase`: The current animation phase (Expanding or Collapsing)
/// - `progress`: Animation progress from 0.0 (start) to 1.0 (end)
///
/// # Returns
/// The interpolated rectangle for the current animation state. Returns an empty
/// rectangle if dimensions would be zero.
pub fn calculate_rect(
	full_rect: Rect,
	_frame_area: Rect,
	phase: AnimationPhase,
	progress: f32,
) -> Rect {
	let progress = progress.clamp(0.0, 1.0);

	let (start_width, start_height, end_width, end_height) = match phase {
		AnimationPhase::Expanding => (
			MIN_WIDTH as f32,
			MIN_HEIGHT as f32,
			full_rect.width as f32,
			full_rect.height as f32,
		),
		AnimationPhase::Collapsing => (
			full_rect.width as f32,
			full_rect.height as f32,
			MIN_WIDTH as f32,
			MIN_HEIGHT as f32,
		),
		_ => return full_rect,
	};

	let current_width_f32 = start_width.lerp(&end_width, progress);
	let current_height_f32 = start_height.lerp(&end_height, progress);

	let current_width = (current_width_f32.round() as u16).max(if progress > 0.0 { 1 } else { 0 });
	let current_height =
		(current_height_f32.round() as u16).max(if progress > 0.0 { 1 } else { 0 });

	let center_x_full = full_rect.x as f32 + (full_rect.width as f32 / 2.0);
	let center_y_full = full_rect.y as f32 + (full_rect.height as f32 / 2.0);

	let current_x = (center_x_full - (current_width as f32 / 2.0)).round() as u16;
	let current_y = (center_y_full - (current_height as f32 / 2.0)).round() as u16;

	if current_width == 0 || current_height == 0 {
		Rect::default()
	} else {
		Rect::new(current_x, current_y, current_width, current_height)
	}
}
