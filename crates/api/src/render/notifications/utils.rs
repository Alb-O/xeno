/// Calculates the intersection of an animated rectangle with a frame area.
///
/// Takes the current animation position and dimensions, clips them to the visible
/// frame boundaries, and returns a rectangle representing the visible portion.
/// Returns an empty rectangle if there's no intersection.
///
/// # Parameters
/// - `current_x`, `current_y`: Current animation position (may be off-screen)
/// - `width`, `height`: Full dimensions of the animated rectangle
/// - `frame_area`: The visible frame area to clip against
///
/// # Returns
/// A `Rect` representing the visible intersection, or an empty `Rect` if fully off-screen.
pub fn clip_rect_to_frame(
	current_x: f32,
	current_y: f32,
	width: u16,
	height: u16,
	frame_area: tome_tui::prelude::Rect,
) -> tome_tui::prelude::Rect {
	use tome_tui::prelude::Rect;

	let anim_x1 = current_x;
	let anim_y1 = current_y;
	let anim_x2 = current_x + width as f32;
	let anim_y2 = current_y + height as f32;
	let frame_x1 = frame_area.x as f32;
	let frame_y1 = frame_area.y as f32;
	let frame_x2 = frame_area.right() as f32;
	let frame_y2 = frame_area.bottom() as f32;

	let intersect_x1 = anim_x1.max(frame_x1);
	let intersect_y1 = anim_y1.max(frame_y1);
	let intersect_x2 = anim_x2.min(frame_x2);
	let intersect_y2 = anim_y2.min(frame_y2);
	let intersect_width = (intersect_x2 - intersect_x1).max(0.0);
	let intersect_height = (intersect_y2 - intersect_y1).max(0.0);

	let final_x = intersect_x1.round() as u16;
	let final_y = intersect_y1.round() as u16;
	let final_width = intersect_width.round() as u16;
	let final_height = intersect_height.round() as u16;

	let final_rect = Rect {
		x: final_x,
		y: final_y,
		width: final_width.min(frame_area.width.saturating_sub(final_x)),
		height: final_height.min(frame_area.height.saturating_sub(final_y)),
	};

	if final_rect.width > 0 && final_rect.height > 0 {
		final_rect
	} else {
		Rect::default()
	}
}
