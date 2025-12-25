use ratatui::prelude::*;
use ratatui::symbols::border;
use ratatui::widgets::Block;

use crate::notifications::types::{Anchor, AnimationPhase, SlideDirection, SlideParams};
use crate::notifications::utils::{color_to_rgb, ease_in_quad, ease_out_quad, lerp};

// Minimum dimensions for expand/collapse animation
const MIN_WIDTH: u16 = 3;
const MIN_HEIGHT: u16 = 3;

// Target color when fully faded out
const FADED_OUT_COLOR: Option<Color> = Some(Color::Black);
// Base color assumed for content text
const BASE_CONTENT_COLOR: Option<Color> = Some(Color::White);

pub fn expand_calculate_rect(
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

	let current_width_f32 = lerp(start_width, end_width, progress);
	let current_height_f32 = lerp(start_height, end_height, progress);

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

pub fn fade_calculate_rect(
	full_rect: Rect,
	_frame_area: Rect,
	_phase: AnimationPhase,
	_progress: f32,
) -> Rect {
	full_rect
}

pub fn interpolate_color(
	from: Option<Color>,
	to: Option<Color>,
	progress: f32,
	is_fading_in: bool,
) -> Option<Color> {
	let linear_progress = progress.clamp(0.0, 1.0);

	if let (Some((r1, g1, b1)), Some((r2, g2, b2))) = (color_to_rgb(from), color_to_rgb(to)) {
		let eased_progress = if is_fading_in {
			ease_out_quad(linear_progress)
		} else {
			ease_in_quad(linear_progress)
		};

		let r_f = lerp(r1 as f32, r2 as f32, eased_progress);
		let g_f = lerp(g1 as f32, g2 as f32, eased_progress);
		let b_f = lerp(b1 as f32, b2 as f32, eased_progress);

		let min_r = r1.min(r2);
		let max_r = r1.max(r2);
		let min_g = g1.min(g2);
		let max_g = g1.max(g2);
		let min_b = b1.min(b2);
		let max_b = b1.max(b2);

		let r = (r_f.round() as u8).clamp(min_r, max_r);
		let g = (g_f.round() as u8).clamp(min_g, max_g);
		let b = (b_f.round() as u8).clamp(min_b, max_b);

		Some(Color::Rgb(r, g, b))
	} else if linear_progress < 0.5 {
		from
	} else {
		to
	}
}

#[derive(Debug, Clone, Copy)]
pub struct FadeHandler;

impl FadeHandler {
	pub fn interpolate_frame_foreground(
		&self,
		base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color> {
		let is_fading_in = matches!(
			phase,
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding
		);
		let (start_fg, end_fg) = match phase {
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding => {
				(FADED_OUT_COLOR, base_fg)
			}
			AnimationPhase::FadingOut | AnimationPhase::SlidingOut | AnimationPhase::Collapsing => {
				(base_fg, FADED_OUT_COLOR)
			}
			_ => return base_fg,
		};
		interpolate_color(start_fg, end_fg, progress, is_fading_in)
	}

	pub fn interpolate_content_foreground(
		&self,
		_base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color> {
		let is_fading_in = matches!(
			phase,
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding
		);
		let (start_fg, end_fg) = match phase {
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding => {
				(FADED_OUT_COLOR, BASE_CONTENT_COLOR)
			}
			AnimationPhase::FadingOut | AnimationPhase::SlidingOut | AnimationPhase::Collapsing => {
				(BASE_CONTENT_COLOR, FADED_OUT_COLOR)
			}
			_ => return BASE_CONTENT_COLOR,
		};
		interpolate_color(start_fg, end_fg, progress, is_fading_in)
	}
}

pub fn slide_calculate_rect(params: SlideParams) -> Rect {
	let progress = params.progress.clamp(0.0, 1.0);

	let (start_x_f32, start_y_f32, end_x_f32, end_y_f32) = match params.phase {
		AnimationPhase::SlidingIn => {
			let (sx, sy) = params.custom_slide_in_start_pos.unwrap_or_else(|| {
				let dir = resolve_slide_direction(params.slide_direction, params.anchor);
				slide_offscreen_position(params.anchor, dir, params.full_rect, params.frame_area)
			});
			(sx, sy, params.full_rect.x as f32, params.full_rect.y as f32)
		}
		AnimationPhase::SlidingOut => {
			let (ex, ey) = params.custom_slide_out_end_pos.unwrap_or_else(|| {
				let dir = resolve_slide_direction(params.slide_direction, params.anchor);
				slide_offscreen_position(params.anchor, dir, params.full_rect, params.frame_area)
			});
			(params.full_rect.x as f32, params.full_rect.y as f32, ex, ey)
		}
		_ => return params.full_rect,
	};

	let current_x_f32 = lerp(start_x_f32, end_x_f32, progress);
	let current_y_f32 = lerp(start_y_f32, end_y_f32, progress);

	let anim_x1 = current_x_f32;
	let anim_y1 = current_y_f32;
	let anim_x2 = current_x_f32 + params.full_rect.width as f32;
	let anim_y2 = current_y_f32 + params.full_rect.height as f32;
	let frame_x1 = params.frame_area.x as f32;
	let frame_y1 = params.frame_area.y as f32;
	let frame_x2 = params.frame_area.right() as f32;
	let frame_y2 = params.frame_area.bottom() as f32;
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
		width: final_width.min(params.frame_area.width.saturating_sub(final_x)),
		height: final_height.min(params.frame_area.height.saturating_sub(final_y)),
	};

	if final_rect.width > 0 && final_rect.height > 0 {
		final_rect
	} else {
		Rect::default()
	}
}

pub fn resolve_slide_direction(direction: SlideDirection, anchor: Anchor) -> SlideDirection {
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

pub fn slide_offscreen_position(
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

pub fn slide_apply_border_effect<'a>(
	block: Block<'a>,
	params: SlideParams,
	base_set: &border::Set<'a>,
) -> Block<'a> {
	const PROGRESS_OFFSET: f32 = 0.0;

	if params.full_rect.width == 0 || params.full_rect.height == 0 {
		return block;
	}

	let slide_direction = resolve_slide_direction(params.slide_direction, params.anchor);

	let (actual_start_x, actual_start_y, actual_end_x, actual_end_y) = match params.phase {
		AnimationPhase::SlidingIn => {
			let (sx, sy) = params.custom_slide_in_start_pos.unwrap_or_else(|| {
				slide_offscreen_position(
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
				slide_offscreen_position(
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

	let (trigger_start, trigger_end) = match slide_direction {
		SlideDirection::FromRight => {
			let crosses = actual_start_x + width > frame_x2 || actual_end_x + width > frame_x2;
			let trigger_s = if !crosses {
				2.0
			} else {
				let travel_dist = actual_end_x - actual_start_x;
				if travel_dist <= 0.0 {
					0.0
				} else {
					let required_pos = frame_x2 - width;
					let dist_to_reach = required_pos - actual_start_x;
					if dist_to_reach <= 0.0 {
						0.0
					} else {
						(dist_to_reach / travel_dist).clamp(0.0, 1.0)
					}
				}
			};
			let trigger_e = if !crosses {
				0.0
			} else {
				let travel_dist = actual_end_x - actual_start_x;
				if travel_dist >= 0.0 {
					1.0
				} else {
					let required_pos_left_edge = frame_x2 - width - actual_start_x;
					if required_pos_left_edge >= 0.0 {
						1.0
					} else {
						(required_pos_left_edge / travel_dist).clamp(0.0, 1.0)
					}
				}
			};
			(trigger_s, trigger_e)
		}
		SlideDirection::FromLeft => {
			let crosses = actual_start_x < frame_x1 || actual_end_x < frame_x1;
			let trigger_s = if !crosses {
				2.0
			} else {
				let travel_dist = actual_end_x - actual_start_x;
				if travel_dist >= 0.0 {
					0.0
				} else {
					let required_pos = frame_x1;
					let dist_to_reach = required_pos - actual_start_x;
					if dist_to_reach >= 0.0 {
						0.0
					} else {
						(dist_to_reach / travel_dist).clamp(0.0, 1.0)
					}
				}
			};
			let trigger_e = if !crosses {
				0.0
			} else {
				let travel_dist = actual_end_x - actual_start_x;
				if travel_dist <= 0.0 {
					1.0
				} else {
					let required_pos_left_edge = frame_x1 - actual_start_x;
					if required_pos_left_edge <= 0.0 {
						1.0
					} else {
						(required_pos_left_edge / travel_dist).clamp(0.0, 1.0)
					}
				}
			};
			(trigger_s, trigger_e)
		}
		SlideDirection::FromTop => {
			let crosses = actual_start_y < frame_y1 || actual_end_y < frame_y1;
			if crosses { (0.0, 1.0) } else { (2.0, 0.0) }
		}
		SlideDirection::FromBottom => {
			let crosses = actual_start_y + height > frame_y2 || actual_end_y + height > frame_y2;
			if crosses { (0.0, 1.0) } else { (2.0, 0.0) }
		}
		SlideDirection::FromTopLeft => {
			let cx = actual_start_x < frame_x1 || actual_end_x < frame_x1;
			let cy = actual_start_y < frame_y1 || actual_end_y < frame_y1;
			if cx || cy { (0.0, 1.0) } else { (2.0, 0.0) }
		}
		SlideDirection::FromTopRight => {
			let cx = actual_start_x + width > frame_x2 || actual_end_x + width > frame_x2;
			let cy = actual_start_y < frame_y1 || actual_end_y < frame_y1;
			if cx || cy { (0.0, 1.0) } else { (2.0, 0.0) }
		}
		SlideDirection::FromBottomLeft => {
			let cx = actual_start_x < frame_x1 || actual_end_x < frame_x1;
			let cy = actual_start_y + height > frame_y2 || actual_end_y + height > frame_y2;
			if cx || cy { (0.0, 1.0) } else { (2.0, 0.0) }
		}
		SlideDirection::FromBottomRight => {
			let cx = actual_start_x + width > frame_x2 || actual_end_x + width > frame_x2;
			let cy = actual_start_y + height > frame_y2 || actual_end_y + height > frame_y2;
			if cx || cy { (0.0, 1.0) } else { (2.0, 0.0) }
		}
		SlideDirection::Default => (2.0, 0.0),
	};

	let apply_effect = match params.phase {
		AnimationPhase::SlidingIn => params.progress < trigger_end - PROGRESS_OFFSET,
		AnimationPhase::SlidingOut => params.progress >= trigger_start - PROGRESS_OFFSET,
		_ => false,
	};

	if !apply_effect {
		return block;
	}

	let (
		top_left_mod,
		top_right_mod,
		bottom_left_mod,
		bottom_right_mod,
		vertical_left_mod,
		vertical_right_mod,
		horizontal_top_mod,
		horizontal_bottom_mod,
	) = match slide_direction {
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
		_ => return block,
	};

	let custom_set = border::Set {
		top_left: top_left_mod.unwrap_or(base_set.top_left),
		top_right: top_right_mod.unwrap_or(base_set.top_right),
		bottom_left: bottom_left_mod.unwrap_or(base_set.bottom_left),
		bottom_right: bottom_right_mod.unwrap_or(base_set.bottom_right),
		vertical_left: vertical_left_mod.unwrap_or(base_set.vertical_left),
		vertical_right: vertical_right_mod.unwrap_or(base_set.vertical_right),
		horizontal_top: horizontal_top_mod.unwrap_or(base_set.horizontal_top),
		horizontal_bottom: horizontal_bottom_mod.unwrap_or(base_set.horizontal_bottom),
	};
	block.border_set(custom_set)
}
