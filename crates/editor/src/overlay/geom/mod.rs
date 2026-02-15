//! Overlay geometry helpers.
//!
//! Contains pure helpers for deriving inner content rects from surface style
//! decorations (border and padding).

use crate::geometry::Rect;
use crate::window::SurfaceStyle;

pub fn pane_inner_rect(rect: Rect, style: &SurfaceStyle) -> Rect {
	let border_left = u16::from(style.border);
	let border_right = u16::from(style.border);
	let border_top = u16::from(style.border);
	let border_bottom = u16::from(style.border);

	let x = rect.x.saturating_add(border_left).saturating_add(style.padding.left);
	let y = rect.y.saturating_add(border_top).saturating_add(style.padding.top);
	let horizontal = border_left
		.saturating_add(border_right)
		.saturating_add(style.padding.left)
		.saturating_add(style.padding.right);
	let vertical = border_top
		.saturating_add(border_bottom)
		.saturating_add(style.padding.top)
		.saturating_add(style.padding.bottom);

	Rect::new(x, y, rect.width.saturating_sub(horizontal), rect.height.saturating_sub(vertical))
}

#[cfg(test)]
mod tests;
