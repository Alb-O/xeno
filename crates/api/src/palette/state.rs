//! Command palette state and lifecycle.

use xeno_tui::layout::Rect;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::BufferId;
use crate::window::{FloatingStyle, WindowId};

/// Active command palette instance.
#[derive(Debug)]
pub struct Palette {
	/// The floating window containing the input buffer.
	pub window_id: WindowId,
	/// The scratch buffer used for input.
	pub buffer_id: BufferId,
}

/// Palette lifecycle state.
#[derive(Debug, Default)]
pub enum PaletteState {
	/// No palette open.
	#[default]
	Closed,
	/// Palette is open and accepting input.
	Open(Palette),
}

impl PaletteState {
	/// Returns true if the palette is open.
	pub fn is_open(&self) -> bool {
		matches!(self, Self::Open(_))
	}

	/// Returns the active palette, if open.
	pub fn active(&self) -> Option<&Palette> {
		match self {
			Self::Open(p) => Some(p),
			Self::Closed => None,
		}
	}

	/// Returns the window ID if palette is open.
	pub fn window_id(&self) -> Option<WindowId> {
		self.active().map(|p| p.window_id)
	}

	/// Returns the buffer ID if palette is open.
	pub fn buffer_id(&self) -> Option<BufferId> {
		self.active().map(|p| p.buffer_id)
	}
}

/// Default floating style for the command palette.
pub fn palette_style() -> FloatingStyle {
	FloatingStyle {
		border: true,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: None,
	}
}

/// Computes the palette rectangle centered horizontally near the top.
pub fn palette_rect(screen_width: u16, screen_height: u16) -> Rect {
	let width = screen_width.saturating_sub(20).clamp(40, 80);
	let height = 3; // Border top + content + border bottom (padding is internal)
	let x = (screen_width.saturating_sub(width)) / 2;
	let y = screen_height / 5;

	Rect::new(x, y, width, height)
}
