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
	/// Resolves the policy into a concrete [`Rect`] relative to the screen bounds.
	///
	/// # Behavior
	///
	/// This method promotes all coordinates to `u32` internally to prevent arithmetic overflow
	/// before clamping the result back to `u16` screen bounds.
	///
	/// - `TopCenter`: Uses a "shift-to-fit" strategy. It attempts to center the window
	///   at the requested Y fraction. If the window would extend off the bottom of the screen,
	///   it shifts the origin up to preserve the requested height. It only shrinks the height
	///   if the screen is too small to fit the window at all.
	/// - `Below`: Uses an "intersection" strategy. It calculates the requested position
	///   and crops the result to the intersection with the screen. If the intersection is
	///   empty, it returns `None`.
	///
	/// # Returns
	///
	/// Returns `None` if:
	/// - The screen has zero width or height.
	/// - A required anchor role is missing (for `Below`).
	/// - The resolved area has zero area or is completely out of bounds.
	/// - The `TopCenter` denominator is zero.
	pub fn resolve_opt(&self, screen: Rect, roles: &HashMap<WindowRole, Rect>) -> Option<Rect> {
		if screen.width == 0 || screen.height == 0 {
			return None;
		}

		let sx = u32::from(screen.x);
		let sy = u32::from(screen.y);
		let sw = u32::from(screen.width);
		let sh = u32::from(screen.height);

		match self {
			Self::TopCenter {
				width_percent,
				max_width,
				min_width,
				y_frac,
				height,
			} => {
				if y_frac.1 == 0 {
					return None;
				}

				let w_pct = u32::from(*width_percent);
				let max_w = u32::from(*max_width);
				let min_w = u32::from(*min_width);
				let fixed_h = u32::from(*height);

				let (safe_min, safe_max) = if min_w > max_w {
					(max_w, min_w)
				} else {
					(min_w, max_w)
				};

				let target_w = (sw.saturating_mul(w_pct) / 100).clamp(safe_min, safe_max);
				let width = target_w.min(sw);
				let height = fixed_h.min(sh);

				let x = sx + (sw.saturating_sub(width) / 2);

				let y_base = sh.saturating_mul(u32::from(y_frac.0)) / u32::from(y_frac.1);
				let raw_y = sy + y_base;

				// Shift up if hitting bottom
				let max_y = (sy + sh).saturating_sub(height);
				let y = raw_y.min(max_y);

				if width == 0 || height == 0 {
					None
				} else {
					Some(Rect::new(x as u16, y as u16, width as u16, height as u16))
				}
			}
			Self::Below(role, offset_y, height) => {
				let anchor = roles.get(role)?;
				let y = u32::from(anchor.y) + u32::from(anchor.height) + u32::from(*offset_y);

				Self::intersect(
					sx,
					sy,
					sw,
					sh,
					u32::from(anchor.x),
					y,
					u32::from(anchor.width),
					u32::from(*height),
				)
			}
		}
	}

	/// Computes the intersection of two rectangles in `u32` space.
	///
	/// Returns `None` if the intersection is empty or invalid.
	fn intersect(
		sx: u32,
		sy: u32,
		sw: u32,
		sh: u32,
		rx: u32,
		ry: u32,
		rw: u32,
		rh: u32,
	) -> Option<Rect> {
		let x = sx.max(rx);
		let y = sy.max(ry);
		let right = (sx + sw).min(rx + rw);
		let bottom = (sy + sh).min(ry + rh);

		if x >= right || y >= bottom {
			return None;
		}

		let width = right - x;
		let height = bottom - y;

		if width == 0 || height == 0 {
			return None;
		}

		Some(Rect::new(x as u16, y as u16, width as u16, height as u16))
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
	}

	#[test]
	fn test_rect_policy_overflow_protection() {
		let screen = Rect::new(0, 0, 100, 50);
		let roles = HashMap::new();

		// Case: width_percent > 100
		let policy_overflow_pct = RectPolicy::TopCenter {
			width_percent: 200,
			max_width: 500,
			min_width: 20,
			y_frac: (1, 4),
			height: 10,
		};
		// Should be clamped to screen width (100)
		let rect = policy_overflow_pct.resolve_opt(screen, &roles).unwrap();
		assert_eq!(rect.width, 100);
		assert_eq!(rect.x, 0);

		// Case: huge y_frac to simulate overflow/wrapping if u16 was used
		let policy_huge_y = RectPolicy::TopCenter {
			width_percent: 50,
			max_width: 80,
			min_width: 20,
			y_frac: (1000, 1), // 50 * 1000 = 50000, which fits in u16 but is way off screen
			height: 10,
		};
		// Should now clamp to screen_bottom - height (50 - 10 = 40)
		let rect = policy_huge_y.resolve_opt(screen, &roles).unwrap();
		assert_eq!(rect.y, 40);
		assert_eq!(rect.height, 10);
	}

	#[test]
	fn test_rect_policy_div_by_zero() {
		let screen = Rect::new(0, 0, 100, 50);
		let roles = HashMap::new();
		let policy = RectPolicy::TopCenter {
			width_percent: 50,
			max_width: 80,
			min_width: 20,
			y_frac: (1, 0), // Division by zero
			height: 10,
		};
		assert!(policy.resolve_opt(screen, &roles).is_none());
	}

	#[test]
	fn test_rect_policy_min_gt_max() {
		let screen = Rect::new(0, 0, 100, 50);
		let roles = HashMap::new();
		let policy = RectPolicy::TopCenter {
			width_percent: 50,
			max_width: 20, // max < min
			min_width: 80,
			y_frac: (1, 4),
			height: 10,
		};
		// Should swap min/max effectively, so min=20, max=80
		// width_percent 50 of 100 is 50. 50 is between 20 and 80.
		let rect = policy.resolve_opt(screen, &roles).unwrap();
		assert_eq!(rect.width, 50);
	}

	#[test]
	fn test_rect_policy_below_clamping() {
		let screen = Rect::new(0, 0, 100, 50);
		let mut roles = HashMap::new();

		// Anchor almost off screen at bottom
		roles.insert(WindowRole::Input, Rect::new(10, 45, 80, 5));

		let policy = RectPolicy::Below(WindowRole::Input, 2, 10);
		// y = 45 + 5 + 2 = 52. Screen h = 50. 52 >= 50. Should be None.
		assert!(policy.resolve_opt(screen, &roles).is_none());

		// Test horizontal clamping for Below
		// Anchor is wider than screen?
		roles.insert(WindowRole::Custom("Wide"), Rect::new(0, 10, 200, 10)); // 200 width
		let policy_wide = RectPolicy::Below(WindowRole::Custom("Wide"), 5, 10);

		// Should clamp width to screen width (100)
		let rect = policy_wide.resolve_opt(screen, &roles).unwrap();
		assert_eq!(rect.width, 100);
		assert_eq!(rect.x, 0);
	}

	#[test]
	fn test_screen_offset_handling() {
		let screen = Rect::new(10, 10, 100, 50); // Screen starts at 10,10
		let roles = HashMap::new();
		let policy = RectPolicy::TopCenter {
			width_percent: 50, // 50 chars
			max_width: 80,
			min_width: 20,
			y_frac: (0, 1), // Top
			height: 10,
		};

		let rect = policy.resolve_opt(screen, &roles).unwrap();
		// Width 50. Centered in 100 is offset 25.
		// X should be screen.x (10) + 25 = 35.
		assert_eq!(rect.x, 35);
		// Y should be screen.y (10) + 0 = 10.
		assert_eq!(rect.y, 10);
	}
}
