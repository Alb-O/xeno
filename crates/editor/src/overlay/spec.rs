use std::collections::HashMap;

use xeno_registry::options::OptionValue;
use xeno_tui::layout::Rect;

use crate::window::{FloatingStyle, GutterSelector};

/// Declarative specification for an overlay's UI layout.
///
/// An `OverlayUiSpec` defines the primary input window and any auxiliary
/// windows (list, preview, etc.) that should be spawned for a session.
#[derive(Debug, Clone)]
pub struct OverlayUiSpec {
	/// Optional title to display in the primary window border.
	pub title: Option<String>,
	/// Gutter configuration for the primary window.
	pub gutter: GutterSelector,
	/// Positioning policy for the primary window.
	pub rect: RectPolicy,
	/// Visual style (border, padding, shadow) for the primary window.
	pub style: FloatingStyle,
	/// List of auxiliary windows to spawn.
	pub windows: Vec<WindowSpec>,
}

/// Specification for an auxiliary window in an overlay.
#[derive(Debug, Clone)]
pub struct WindowSpec {
	/// Logical role of this window (used for relative positioning).
	pub role: WindowRole,
	/// Positioning policy for this window.
	pub rect: RectPolicy,
	/// Visual style for this window.
	pub style: FloatingStyle,
	/// Buffer-local options to apply to this window's scratch buffer.
	pub buffer_options: HashMap<String, OptionValue>,
	/// Whether to dismiss the entire overlay when this window loses focus.
	pub dismiss_on_blur: bool,
	/// Whether this window should stay on top and capture input.
	pub sticky: bool,
	/// Gutter configuration for this window.
	pub gutter: GutterSelector,
}

/// Logical role of a window within an overlay interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowRole {
	/// The primary user input field.
	Input,
	/// A list of selectable items (e.g. command list, file list).
	List,
	/// A preview of the selected item's content.
	Preview,
	/// Custom application-specific role.
	Custom,
}

/// Policy for calculating the screen area of an overlay window.
#[derive(Debug, Clone)]
pub enum RectPolicy {
	/// Centered horizontally at a specific vertical fractional position.
	TopCenter {
		/// Width as a percentage of total screen width.
		width_percent: u16,
		/// Maximum allowed width in characters.
		max_width: u16,
		/// Minimum required width in characters.
		min_width: u16,
		/// Vertical position as a fraction (numerator, denominator).
		y_frac: (u16, u16),
		/// Fixed height in characters.
		height: u16,
	},
	/// Positioned directly below another window.
	Below(WindowRole, u16, u16),
}

impl RectPolicy {
	/// Resolves the policy into a concrete [`Rect`] based on the screen size
	/// and already resolved window roles.
	pub fn resolve(&self, screen: Rect, roles: &HashMap<WindowRole, Rect>) -> Rect {
		match self {
			Self::TopCenter {
				width_percent,
				max_width,
				min_width,
				y_frac,
				height,
			} => {
				let width = (screen.width * width_percent / 100).clamp(*min_width, *max_width);
				let x = (screen.width.saturating_sub(width)) / 2;
				let y = screen.height * y_frac.0 / y_frac.1;
				Rect::new(x, y, width, *height)
			}
			Self::Below(role, offset_y, height) => {
				if let Some(r) = roles.get(role) {
					Rect::new(r.x, r.y + r.height + offset_y, r.width, *height)
				} else {
					Rect::default()
				}
			}
		}
	}
}
