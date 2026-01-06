//! Info popup panels for displaying documentation and contextual information.
//!
//! Info popups are read-only floating buffers used for:
//! - LSP hover documentation
//! - Command completion info in the command palette
//! - Any contextual help or documentation display
//!
//! They reuse the buffer renderer for syntax highlighting and text wrapping.

use xeno_tui::layout::Rect;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::BufferId;
use crate::window::{FloatingStyle, WindowId};

/// Unique identifier for an info popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InfoPopupId(pub u64);

/// An active info popup instance.
#[derive(Debug)]
pub struct InfoPopup {
	/// Unique identifier for this popup.
	pub id: InfoPopupId,
	/// The floating window containing the content.
	pub window_id: WindowId,
	/// The read-only buffer displaying content.
	pub buffer_id: BufferId,
	/// Anchor position for the popup (where it should appear relative to).
	pub anchor: PopupAnchor,
}

/// Anchor point for positioning info popups.
#[derive(Debug, Clone, Copy, Default)]
pub enum PopupAnchor {
	/// Position relative to cursor in the active buffer.
	#[default]
	Cursor,
	/// Position relative to a specific screen coordinate.
	Point { x: u16, y: u16 },
	/// Position relative to another window (e.g., completion menu).
	Window(WindowId),
}

/// Default floating style for info popups.
///
/// Uses the same stripe border as command palette and notifications
/// for visual consistency.
pub fn info_popup_style() -> FloatingStyle {
	FloatingStyle {
		border: true,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: None,
	}
}

/// Computes the popup rectangle based on anchor and content size.
///
/// Positions below/right of anchor, flipping if insufficient space.
pub fn compute_popup_rect(
	anchor: PopupAnchor,
	content_width: u16,
	content_height: u16,
	screen_width: u16,
	screen_height: u16,
	cursor_screen_pos: Option<(u16, u16)>,
) -> Rect {
	let width = content_width.saturating_add(2).min(screen_width.saturating_sub(4));
	let height = content_height.saturating_add(2).min(screen_height.saturating_sub(2));

	let (anchor_x, anchor_y) = match anchor {
		PopupAnchor::Cursor => cursor_screen_pos.unwrap_or((screen_width / 2, screen_height / 2)),
		PopupAnchor::Point { x, y } => (x, y),
		PopupAnchor::Window(_) => (screen_width / 2, screen_height / 2), // TODO: look up window position
	};

	let mut x = anchor_x.saturating_add(1);
	let mut y = anchor_y.saturating_add(1);

	if x.saturating_add(width) > screen_width {
		x = anchor_x.saturating_sub(width).saturating_sub(1);
	}
	if y.saturating_add(height) > screen_height {
		y = anchor_y.saturating_sub(height);
	}

	x = x.min(screen_width.saturating_sub(width));
	y = y.min(screen_height.saturating_sub(height));

	Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn popup_rect_positions_below_cursor() {
		let rect = compute_popup_rect(PopupAnchor::Cursor, 20, 5, 80, 24, Some((10, 5)));
		assert!(rect.x > 10);
		assert!(rect.y > 5);
	}

	#[test]
	fn popup_rect_flips_when_near_edge() {
		let rect = compute_popup_rect(PopupAnchor::Cursor, 20, 5, 80, 24, Some((75, 20)));
		assert!(rect.x < 75);
		assert!(rect.y < 20);
	}

	#[test]
	fn popup_rect_clamps_to_screen() {
		let rect = compute_popup_rect(PopupAnchor::Point { x: 0, y: 0 }, 100, 30, 80, 24, None);
		assert!(rect.x + rect.width <= 80);
		assert!(rect.y + rect.height <= 24);
	}
}
