use tome_tui::animation::Animatable;
use tome_tui::prelude::*;
use tome_tui::symbols::border;
use tome_tui::widgets::Block;
use tome_manifest::notifications::{Anchor, AnimationPhase, SlideDirection};

use super::border::calculate_triggers;
use crate::render::notifications::types::SlideParams;
use crate::render::notifications::utils::clip_rect_to_frame;

/// Calculates the rectangle for slide animation at a given progress.
///
/// This function handles both sliding in (from off-screen to final position)
/// and sliding out (from final position to off-screen). The slide direction
/// determines which edge of the screen the animation originates from or exits to.
///
/// # Parameters
/// - `params`: Complete slide animation parameters
///
/// # Returns
/// The interpolated and clipped rectangle for the current animation state.
/// Returns an empty rectangle if completely off-screen or has zero dimensions.
pub fn calculate_rect(params: SlideParams) -> Rect {
	let progress = params.progress.clamp(0.0, 1.0);

	let (start_x_f32, start_y_f32, end_x_f32, end_y_f32) = match params.phase {
		AnimationPhase::SlidingIn => {
			let (sx, sy) = params.custom_slide_in_start_pos.unwrap_or_else(|| {
				let dir = resolve_direction(params.slide_direction, params.anchor);
				offscreen_position(params.anchor, dir, params.full_rect, params.frame_area)
			});
			(sx, sy, params.full_rect.x as f32, params.full_rect.y as f32)
		}
		AnimationPhase::SlidingOut => {
			let (ex, ey) = params.custom_slide_out_end_pos.unwrap_or_else(|| {
				let dir = resolve_direction(params.slide_direction, params.anchor);
				offscreen_position(params.anchor, dir, params.full_rect, params.frame_area)
			});
			(params.full_rect.x as f32, params.full_rect.y as f32, ex, ey)
		}
		_ => return params.full_rect,
	};

	let current_x_f32 = start_x_f32.lerp(&end_x_f32, progress);
	let current_y_f32 = start_y_f32.lerp(&end_y_f32, progress);

	clip_rect_to_frame(
		current_x_f32,
		current_y_f32,
		params.full_rect.width,
		params.full_rect.height,
		params.frame_area,
	)
}

/// Resolves the slide direction from a default based on anchor position.
///
/// If the direction is `Default`, determines the natural slide direction for
/// the given anchor (e.g., top-left anchor slides from top-left).
///
/// # Parameters
/// - `direction`: The requested slide direction
/// - `anchor`: The notification anchor position
///
/// # Returns
/// The resolved slide direction (never `Default`).
pub fn resolve_direction(direction: SlideDirection, anchor: Anchor) -> SlideDirection {
	if direction != SlideDirection::Default {
		return direction;
	}
	match anchor {
		Anchor::TopLeft => SlideDirection::FromTopLeft,
		Anchor::TopCenter => SlideDirection::FromTop,
		Anchor::TopRight => SlideDirection::FromTopRight,
		Anchor::MiddleLeft => SlideDirection::FromLeft,
		Anchor::MiddleCenter => SlideDirection::FromLeft,
		Anchor::MiddleRight => SlideDirection::FromRight,
		Anchor::BottomLeft => SlideDirection::FromBottomLeft,
		Anchor::BottomCenter => SlideDirection::FromBottom,
		Anchor::BottomRight => SlideDirection::FromBottomRight,
	}
}

/// Calculates the off-screen starting or ending position for slide animations.
///
/// Places the notification just beyond the visible frame edge in the direction
/// of the slide, with a small margin to ensure it's fully hidden.
///
/// # Parameters
/// - `_anchor`: The anchor position (reserved for future use)
/// - `slide_direction`: The direction of the slide
/// - `full_rect`: The full notification rectangle
/// - `frame_area`: The containing frame area
///
/// # Returns
/// The (x, y) position in floating-point coordinates for smooth interpolation.
pub fn offscreen_position(
	_anchor: Anchor,
	slide_direction: SlideDirection,
	full_rect: Rect,
	frame_area: Rect,
) -> (f32, f32) {
	const EDGE_MARGIN: i16 = 1;
	let width = full_rect.width as i16;
	let height = full_rect.height as i16;
	let frame_x = frame_area.x as i16;
	let frame_y = frame_area.y as i16;
	let frame_right = frame_area.right() as i16;
	let frame_bottom = frame_area.bottom() as i16;
	let full_x = full_rect.x as i16;
	let full_y = full_rect.y as i16;

	let start_x = match slide_direction {
		SlideDirection::FromLeft | SlideDirection::FromTopLeft | SlideDirection::FromBottomLeft => {
			frame_x.saturating_sub(width).saturating_sub(EDGE_MARGIN)
		}
		SlideDirection::FromRight
		| SlideDirection::FromTopRight
		| SlideDirection::FromBottomRight => frame_right.saturating_add(EDGE_MARGIN),
		_ => full_x,
	};
	let start_y = match slide_direction {
		SlideDirection::FromTop | SlideDirection::FromTopLeft | SlideDirection::FromTopRight => {
			frame_y.saturating_sub(height).saturating_sub(EDGE_MARGIN)
		}
		SlideDirection::FromBottom
		| SlideDirection::FromBottomLeft
		| SlideDirection::FromBottomRight => frame_bottom.saturating_add(EDGE_MARGIN),
		_ => full_y,
	};
	(start_x as f32, start_y as f32)
}

/// Applies border modifications for slide-in/out edge effects.
///
/// During slide animations, the border characters are modified to create a clean
/// appearance as the notification enters or exits the frame. This prevents artifacts
/// where corner characters appear before they should.
///
/// # Parameters
/// - `block`: The notification block to modify
/// - `params`: Complete slide animation parameters
/// - `base_set`: The base border character set
///
/// # Returns
/// The modified block with custom border characters, or the original block if
/// no modifications are needed.
pub fn apply_border_effect<'a>(
	block: Block<'a>,
	params: SlideParams,
	base_set: &border::Set<'a>,
) -> Block<'a> {
	const PROGRESS_OFFSET: f32 = 0.0;

	if params.full_rect.width == 0 || params.full_rect.height == 0 {
		return block;
	}

	let slide_direction = resolve_direction(params.slide_direction, params.anchor);

	let (actual_start_x, actual_start_y, actual_end_x, actual_end_y) = match params.phase {
		AnimationPhase::SlidingIn => {
			let (sx, sy) = params.custom_slide_in_start_pos.unwrap_or_else(|| {
				offscreen_position(
					params.anchor,
					slide_direction,
					params.full_rect,
					params.frame_area,
				)
			});
			(sx, sy, params.full_rect.x as f32, params.full_rect.y as f32)
		}
		AnimationPhase::SlidingOut => {
			let (ex, ey) = params.custom_slide_out_end_pos.unwrap_or_else(|| {
				offscreen_position(
					params.anchor,
					slide_direction,
					params.full_rect,
					params.frame_area,
				)
			});
			(params.full_rect.x as f32, params.full_rect.y as f32, ex, ey)
		}
		_ => return block,
	};

	let frame_x1 = params.frame_area.x as f32;
	let frame_y1 = params.frame_area.y as f32;
	let frame_x2 = params.frame_area.right() as f32;
	let frame_y2 = params.frame_area.bottom() as f32;
	let width = params.full_rect.width as f32;
	let height = params.full_rect.height as f32;

	let (trigger_start, trigger_end) = calculate_triggers(
		slide_direction,
		actual_start_x,
		actual_start_y,
		actual_end_x,
		actual_end_y,
		frame_x1,
		frame_y1,
		frame_x2,
		frame_y2,
		width,
		height,
	);

	let apply_effect = match params.phase {
		AnimationPhase::SlidingIn => params.progress < trigger_end - PROGRESS_OFFSET,
		AnimationPhase::SlidingOut => params.progress >= trigger_start - PROGRESS_OFFSET,
		_ => false,
	};

	if !apply_effect {
		return block;
	}

	let custom_set = build_custom_border_set(slide_direction, base_set);
	block.border_set(custom_set)
}

/// Builds a custom border character set for the given slide direction.
///
/// Replaces specific border characters with spaces to create clean edge effects
/// as the notification slides in or out.
fn build_custom_border_set<'a>(
	slide_direction: SlideDirection,
	base_set: &border::Set<'a>,
) -> border::Set<'a> {
	let (tl, tr, bl, br, vl, vr, ht, hb) = match slide_direction {
		SlideDirection::FromRight => (
			None,
			Some(base_set.horizontal_top),
			None,
			Some(base_set.horizontal_bottom),
			None,
			Some(" "),
			None,
			None,
		),
		SlideDirection::FromLeft => (
			Some(base_set.horizontal_top),
			None,
			Some(base_set.horizontal_bottom),
			None,
			Some(" "),
			None,
			None,
			None,
		),
		SlideDirection::FromTop => (
			Some(base_set.vertical_left),
			Some(base_set.vertical_right),
			None,
			None,
			None,
			None,
			Some(" "),
			None,
		),
		SlideDirection::FromBottom => (
			None,
			None,
			Some(base_set.vertical_left),
			Some(base_set.vertical_right),
			None,
			None,
			None,
			Some(" "),
		),
		SlideDirection::FromTopLeft => (
			Some(base_set.horizontal_top),
			Some(base_set.vertical_right),
			Some(base_set.horizontal_bottom),
			None,
			Some(" "),
			None,
			Some(" "),
			None,
		),
		SlideDirection::FromTopRight => (
			Some(base_set.vertical_left),
			Some(base_set.horizontal_top),
			None,
			Some(base_set.horizontal_bottom),
			None,
			Some(" "),
			Some(" "),
			None,
		),
		SlideDirection::FromBottomLeft => (
			Some(base_set.horizontal_top),
			None,
			Some(base_set.vertical_left),
			Some(base_set.vertical_right),
			Some(" "),
			None,
			None,
			Some(" "),
		),
		SlideDirection::FromBottomRight => (
			None,
			Some(base_set.horizontal_top),
			Some(base_set.vertical_left),
			Some(base_set.horizontal_bottom),
			None,
			Some(" "),
			None,
			Some(" "),
		),
		_ => return *base_set,
	};

	border::Set {
		top_left: tl.unwrap_or(base_set.top_left),
		top_right: tr.unwrap_or(base_set.top_right),
		bottom_left: bl.unwrap_or(base_set.bottom_left),
		bottom_right: br.unwrap_or(base_set.bottom_right),
		vertical_left: vl.unwrap_or(base_set.vertical_left),
		vertical_right: vr.unwrap_or(base_set.vertical_right),
		horizontal_top: ht.unwrap_or(base_set.horizontal_top),
		horizontal_bottom: hb.unwrap_or(base_set.horizontal_bottom),
	}
}
