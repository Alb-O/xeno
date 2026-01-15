//! Window and floating window types.

use xeno_registry::gutter::{GutterCell, GutterLineContext};
use xeno_tui::layout::Rect;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::{BufferId, Layout};

/// Unique identifier for a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub(crate) u64);

impl From<WindowId> for xeno_registry::WindowId {
	fn from(id: WindowId) -> Self {
		xeno_registry::WindowId(id.0)
	}
}

impl From<xeno_registry::WindowId> for WindowId {
	fn from(id: xeno_registry::WindowId) -> Self {
		WindowId(id.0)
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub enum GutterSelector {
	/// Use enabled gutters from registry (default behavior).
	#[default]
	Registry,
	/// Use specific gutters by name.
	Named(&'static [&'static str]),
	/// Hide gutter entirely.
	Hidden,
	/// Single prompt character.
	Prompt(char),
	/// Custom render function.
	Custom {
		width: u16,
		render: fn(&GutterLineContext) -> Option<GutterCell>,
	},
}

/// Window kinds.
pub enum Window {
	/// The base window containing the split tree.
	Base(BaseWindow),
	/// A floating window positioned over content.
	Floating(FloatingWindow),
}

/// The main editor window with split layout.
pub struct BaseWindow {
	pub layout: Layout,
	pub focused_buffer: BufferId,
}

/// A floating window with absolute positioning.
#[derive(Debug, Clone)]
pub struct FloatingWindow {
	pub id: WindowId,
	pub buffer: BufferId,
	pub rect: Rect,
	/// Gutter configuration for this window.
	pub gutter: GutterSelector,
	/// If true, resists losing focus from mouse hover.
	pub sticky: bool,
	/// If true, closes when focus is lost.
	pub dismiss_on_blur: bool,
	/// Visual style (border, shadow, transparency).
	pub style: FloatingStyle,
}

/// Visual style for floating windows.
#[derive(Debug, Clone)]
pub struct FloatingStyle {
	pub border: bool,
	pub border_type: BorderType,
	pub padding: Padding,
	pub shadow: bool,
	pub title: Option<String>,
}

impl Default for FloatingStyle {
	fn default() -> Self {
		Self {
			border: true,
			border_type: BorderType::Rounded,
			padding: Padding::ZERO,
			shadow: false,
			title: None,
		}
	}
}
