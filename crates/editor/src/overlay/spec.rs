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
	Custom(&'static str),
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
	///
	/// Clamps the resulting area to ensure it stays within the screen bounds.
	/// Returns `None` if an anchor role required for relative positioning is missing.
	pub fn resolve_opt(&self, screen: Rect, roles: &HashMap<WindowRole, Rect>) -> Option<Rect> {
		match self {
			Self::TopCenter {
				width_percent,
				max_width,
				min_width,
				y_frac,
				height,
			} => {
				let width = (screen.width * width_percent / 100).clamp(*min_width, *max_width);
				let width = width.min(screen.width);
				let x = (screen.width.saturating_sub(width)) / 2;

				let y_base = screen.height * y_frac.0 / y_frac.1;
				let height = (*height).min(screen.height);
				let y = if y_base + height > screen.height {
					screen.height.saturating_sub(height)
				} else {
					y_base
				};

				Some(Rect::new(x, y, width, height))
			}
			Self::Below(role, offset_y, height) => {
				let r = roles.get(role)?;
				let x = r.x;
				let width = r.width;

				let y_base = r.y + r.height + offset_y;
				let height = (*height).min(screen.height.saturating_sub(y_base));
				if height == 0 {
					return None;
				}

				Some(Rect::new(x, y_base, width, height))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_rect_policy_top_center_clamping() {
		let screen = Rect::new(0, 0, 100, 50);
		let roles = HashMap::new();
		let policy = RectPolicy::TopCenter {
			width_percent: 50,
			max_width: 80,
			min_width: 20,
			y_frac: (1, 4),
			height: 10,
		};

		let rect = policy.resolve_opt(screen, &roles).unwrap();
		assert_eq!(rect.width, 50);
		assert_eq!(rect.x, 25);
		assert_eq!(rect.y, 12);
		assert_eq!(rect.height, 10);

		// Oversized width
		let policy_oversized = RectPolicy::TopCenter {
			width_percent: 150,
			max_width: 200,
			min_width: 20,
			y_frac: (1, 4),
			height: 10,
		};
		let rect_oversized = policy_oversized.resolve_opt(screen, &roles).unwrap();
		assert_eq!(rect_oversized.width, 100);
		assert_eq!(rect_oversized.x, 0);

		// Oversized height
		let policy_tall = RectPolicy::TopCenter {
			width_percent: 50,
			max_width: 80,
			min_width: 20,
			y_frac: (3, 4),
			height: 20,
		};
		let rect_tall = policy_tall.resolve_opt(screen, &roles).unwrap();
		assert!(rect_tall.y + rect_tall.height <= 50);
	}

	#[test]
	fn test_rect_policy_below_missing_anchor() {
		let screen = Rect::new(0, 0, 100, 50);
		let roles = HashMap::new();
		let policy = RectPolicy::Below(WindowRole::Input, 1, 5);

		assert!(policy.resolve_opt(screen, &roles).is_none());
	}

	#[test]
	fn test_rect_policy_below_clamping() {
		let screen = Rect::new(0, 0, 100, 50);
		let mut roles = HashMap::new();
		roles.insert(WindowRole::Input, Rect::new(10, 40, 80, 5));

		let policy = RectPolicy::Below(WindowRole::Input, 2, 10);
		let rect = policy.resolve_opt(screen, &roles).unwrap();

		assert_eq!(rect.x, 10);
		assert_eq!(rect.width, 80);
		assert_eq!(rect.y, 47);
		assert_eq!(rect.height, 3); // 50 - 47
	}
}
