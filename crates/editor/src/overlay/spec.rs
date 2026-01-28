use std::collections::HashMap;

use xeno_registry::options::OptionValue;
use xeno_tui::layout::Rect;

use crate::window::{FloatingStyle, GutterSelector};

#[derive(Debug, Clone)]
pub struct OverlayUiSpec {
	pub title: Option<String>,
	pub gutter: GutterSelector,
	pub rect: RectPolicy,
	pub style: FloatingStyle,
	pub windows: Vec<WindowSpec>,
}

#[derive(Debug, Clone)]
pub struct WindowSpec {
	pub role: WindowRole,
	pub rect: RectPolicy,
	pub style: FloatingStyle,
	pub buffer_options: HashMap<String, OptionValue>,
	pub dismiss_on_blur: bool,
	pub sticky: bool,
	pub gutter: GutterSelector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowRole {
	Input,
	List,
	Preview,
	Custom,
}

#[derive(Debug, Clone)]
pub enum RectPolicy {
	/// Centered horizontally near the top.
	TopCenter {
		width_percent: u16,
		max_width: u16,
		min_width: u16,
		y_frac: (u16, u16),
		height: u16,
	},
	/// Relative to another window role.
	Below(WindowRole, u16, u16),
}

impl RectPolicy {
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
