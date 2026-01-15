//! Floating window helpers.

use xeno_tui::layout::Rect;

use super::types::{FloatingStyle, FloatingWindow, GutterSelector, WindowId};
use crate::buffer::BufferId;

impl FloatingWindow {
	/// Creates a new floating window with default behaviors.
	pub fn new(id: WindowId, buffer: BufferId, rect: Rect, style: FloatingStyle) -> Self {
		Self {
			id,
			buffer,
			rect,
			gutter: GutterSelector::Registry,
			sticky: false,
			dismiss_on_blur: false,
			style,
		}
	}

	/// Returns true if the point lies within the window rect.
	pub fn contains(&self, x: u16, y: u16) -> bool {
		x >= self.rect.x
			&& x < self.rect.x.saturating_add(self.rect.width)
			&& y >= self.rect.y
			&& y < self.rect.y.saturating_add(self.rect.height)
	}

	/// Returns the content area inside the floating window.
	pub fn content_rect(&self) -> Rect {
		if self.style.border {
			Rect {
				x: self.rect.x.saturating_add(1),
				y: self.rect.y.saturating_add(1),
				width: self.rect.width.saturating_sub(2),
				height: self.rect.height.saturating_sub(2),
			}
		} else {
			self.rect
		}
	}
}
