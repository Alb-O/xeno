//! Layout and positioning logic for toast notifications.

use super::super::types::{Anchor, SlideDirection};
use super::state::ToastState;
use crate::animation::{Animatable, Easing};
use crate::layout::{Position, Rect};

/// Vertical spacing between stacked toasts.
pub(super) const STACK_SPACING: u16 = 1;

/// Converts an anchor to its screen position within the given area.
pub(super) fn anchor_position(anchor: Anchor, area: Rect) -> Position {
	match anchor {
		Anchor::TopLeft => Position::new(area.x, area.y),
		Anchor::TopCenter => Position::new(area.x + area.width / 2, area.y),
		Anchor::TopRight => Position::new(area.right().saturating_sub(1), area.y),
		Anchor::MiddleLeft => Position::new(area.x, area.y + area.height / 2),
		Anchor::MiddleCenter => Position::new(area.x + area.width / 2, area.y + area.height / 2),
		Anchor::MiddleRight => {
			Position::new(area.right().saturating_sub(1), area.y + area.height / 2)
		}
		Anchor::BottomLeft => Position::new(area.x, area.bottom().saturating_sub(1)),
		Anchor::BottomCenter => {
			Position::new(area.x + area.width / 2, area.bottom().saturating_sub(1))
		}
		Anchor::BottomRight => Position::new(
			area.right().saturating_sub(1),
			area.bottom().saturating_sub(1),
		),
	}
}

/// Calculates the X position for a toast given anchor and dimensions.
pub(super) fn calculate_x(
	anchor: Anchor,
	anchor_x: u16,
	width: u16,
	margin: u16,
	area: Rect,
) -> u16 {
	let x = match anchor {
		Anchor::TopCenter | Anchor::MiddleCenter | Anchor::BottomCenter => {
			anchor_x.saturating_sub(width / 2)
		}
		Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight => {
			anchor_x.saturating_sub(width).saturating_sub(margin)
		}
		_ => anchor_x.saturating_add(margin),
	};
	x.clamp(area.x, area.right().saturating_sub(width))
}

/// Calculates the Y position for a toast given anchor and dimensions.
pub(super) fn calculate_y(
	anchor: Anchor,
	anchor_y: u16,
	height: u16,
	margin: u16,
	area: Rect,
) -> u16 {
	let y = match anchor {
		Anchor::MiddleLeft | Anchor::MiddleCenter | Anchor::MiddleRight => {
			anchor_y.saturating_sub(height / 2)
		}
		Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => {
			anchor_y.saturating_sub(height).saturating_sub(margin)
		}
		_ => anchor_y.saturating_add(margin),
	};
	y.clamp(area.y, area.bottom().saturating_sub(height))
}

/// Returns the width needed to display the stack counter (e.g., "⨯12").
pub(super) fn stack_counter_width(stack_count: u32) -> u16 {
	if stack_count <= 1 {
		return 0;
	}
	let digits = stack_count.checked_ilog10().unwrap_or(0) + 1;
	1 + digits as u16
}

/// Computes the toast dimensions based on content and constraints.
pub(super) fn calculate_toast_size(
	toast: &super::super::toast::Toast,
	area: Rect,
	stack_count: u32,
) -> (u16, u16) {
	use super::super::types::SizeConstraint;

	let max_width = toast
		.max_width
		.map(|c| match c {
			SizeConstraint::Cells(w) => w.min(area.width),
			SizeConstraint::Percent(p) => {
				((area.width as f32 * p.clamp(0.0, 1.0)).ceil() as u16).max(1)
			}
		})
		.unwrap_or(area.width);

	let max_height = toast
		.max_height
		.map(|c| match c {
			SizeConstraint::Cells(h) => h.min(area.height),
			SizeConstraint::Percent(p) => {
				((area.height as f32 * p.clamp(0.0, 1.0)).ceil() as u16).max(1)
			}
		})
		.unwrap_or(area.height);

	let padding_h = toast.padding.left + toast.padding.right;
	let padding_v = toast.padding.top + toast.padding.bottom;
	let icon_width = toast.icon_column_width();
	let counter_width = stack_counter_width(stack_count);

	let content_width = toast
		.content
		.lines()
		.map(|l| l.chars().count())
		.max()
		.unwrap_or(0) as u16;

	let width = (content_width.max(counter_width) + icon_width + 2 + padding_h)
		.max(3)
		.min(max_width);

	// Account for text wrapping when calculating height
	let inner_width = width.saturating_sub(2 + padding_h + icon_width);
	let wrapped_lines: u16 = if inner_width > 0 {
		toast
			.content
			.lines()
			.map(|line| {
				let len = line.chars().count() as u16;
				if len == 0 {
					1
				} else {
					len.div_ceil(inner_width)
				}
			})
			.sum::<u16>()
			.max(1)
	} else {
		1
	};

	let extra_lines = if stack_count > 1 { 1 } else { 0 };
	let height = (wrapped_lines + extra_lines + 2 + padding_v)
		.max(3)
		.min(max_height);

	(width, height)
}

/// Applies animation transforms to compute the current visible rect.
pub(super) fn apply_animation(state: &ToastState, full_rect: Rect, area: Rect) -> Rect {
	use super::super::types::{Animation, AnimationPhase};

	let progress = Easing::EaseOut.apply(state.progress);

	match state.phase {
		AnimationPhase::Pending => Rect::default(),
		AnimationPhase::Dwelling | AnimationPhase::Finished => full_rect,
		AnimationPhase::Entering => match state.toast.animation {
			Animation::Fade => full_rect,
			Animation::Slide => offscreen_rect(state, full_rect, area).lerp(&full_rect, progress),
			Animation::ExpandCollapse => {
				let cx = full_rect.x + full_rect.width / 2;
				let cy = full_rect.y + full_rect.height / 2;
				Rect::new(cx, cy, 1, 1).lerp(&full_rect, progress)
			}
		},
		AnimationPhase::Exiting => {
			let progress = Easing::EaseIn.apply(state.progress);
			match state.toast.animation {
				Animation::Fade => full_rect,
				Animation::Slide => {
					full_rect.lerp(&offscreen_rect(state, full_rect, area), progress)
				}
				Animation::ExpandCollapse => {
					let cx = full_rect.x + full_rect.width / 2;
					let cy = full_rect.y + full_rect.height / 2;
					full_rect.lerp(&Rect::new(cx, cy, 1, 1), progress)
				}
			}
		}
	}
}

/// Calculates the offscreen position for slide animations.
pub(super) fn offscreen_rect(state: &ToastState, full_rect: Rect, area: Rect) -> Rect {
	let direction = match state.toast.slide_direction {
		SlideDirection::Auto => default_slide_direction(state.toast.anchor),
		other => other,
	};

	let (dx, dy): (i32, i32) = match direction {
		SlideDirection::FromTop => (0, -(full_rect.height as i32 + 2)),
		SlideDirection::FromBottom => (0, area.height as i32 + 2),
		SlideDirection::FromLeft => (-(full_rect.width as i32 + 2), 0),
		SlideDirection::FromRight => (area.width as i32 + 2, 0),
		SlideDirection::FromTopLeft => (
			-(full_rect.width as i32 + 2),
			-(full_rect.height as i32 + 2),
		),
		SlideDirection::FromTopRight => (area.width as i32 + 2, -(full_rect.height as i32 + 2)),
		SlideDirection::FromBottomLeft => (-(full_rect.width as i32 + 2), area.height as i32 + 2),
		SlideDirection::FromBottomRight => (area.width as i32 + 2, area.height as i32 + 2),
		SlideDirection::Auto => (0, 0),
	};

	Rect::new(
		(full_rect.x as i32 + dx).max(0) as u16,
		(full_rect.y as i32 + dy).max(0) as u16,
		full_rect.width,
		full_rect.height,
	)
}

/// Determines the default slide direction for a given anchor.
pub(super) fn default_slide_direction(anchor: Anchor) -> SlideDirection {
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
