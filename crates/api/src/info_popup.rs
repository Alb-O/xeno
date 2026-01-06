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
	/// Centered in the document area.
	#[default]
	Center,
	/// Position relative to a specific screen coordinate (top-left of popup).
	Point { x: u16, y: u16 },
	/// Position adjacent to another window (e.g., next to completion menu).
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
/// Clamps to stay within bounds.
pub fn compute_popup_rect(
	anchor: PopupAnchor,
	content_width: u16,
	content_height: u16,
	bounds: Rect,
) -> Rect {
	let width = content_width.saturating_add(2).min(bounds.width.saturating_sub(4));
	let height = content_height.saturating_add(2).min(bounds.height.saturating_sub(2));

	let (x, y) = match anchor {
		PopupAnchor::Center => (
			bounds.x + bounds.width.saturating_sub(width) / 2,
			bounds.y + bounds.height.saturating_sub(height) / 2,
		),
		PopupAnchor::Point { x, y } => (
			x.max(bounds.x).min(bounds.x + bounds.width.saturating_sub(width)),
			y.max(bounds.y).min(bounds.y + bounds.height.saturating_sub(height)),
		),
		PopupAnchor::Window(_) => (
			bounds.x + bounds.width.saturating_sub(width) / 2,
			bounds.y + bounds.height.saturating_sub(height) / 2,
		), // TODO: position adjacent to window
	};

	Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn popup_rect_centers_in_bounds() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(PopupAnchor::Center, 20, 5, bounds);
		assert!(rect.x > bounds.x);
		assert!(rect.y > bounds.y);
		assert!(rect.x + rect.width < bounds.x + bounds.width);
		assert!(rect.y + rect.height < bounds.y + bounds.height);
	}

	#[test]
	fn popup_rect_clamps_point_to_bounds() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(PopupAnchor::Point { x: 100, y: 100 }, 20, 5, bounds);
		assert!(rect.x + rect.width <= bounds.x + bounds.width);
		assert!(rect.y + rect.height <= bounds.y + bounds.height);
	}

	#[test]
	fn popup_rect_respects_point_position() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(PopupAnchor::Point { x: 10, y: 5 }, 20, 5, bounds);
		assert_eq!(rect.x, 10);
		assert_eq!(rect.y, 5);
	}
}
