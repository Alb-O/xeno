//! Window and surface style types.

use xeno_registry::gutter::{GutterCell, GutterLineContext};

use crate::buffer::{Layout, ViewId};

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
}

impl Window {
	pub fn buffer(&self) -> ViewId {
		match self {
			Window::Base(b) => b.focused_buffer,
		}
	}
}

/// The main editor window with split layout.
pub struct BaseWindow {
	pub layout: Layout,
	pub focused_buffer: ViewId,
}

/// Visual style for overlay surfaces.
#[derive(Debug, Clone)]
pub struct SurfaceStyle {
	pub border: bool,
	pub border_type: SurfaceBorder,
	pub padding: SurfacePadding,
	pub shadow: bool,
	pub title: Option<String>,
}

impl Default for SurfaceStyle {
	fn default() -> Self {
		Self {
			border: true,
			border_type: SurfaceBorder::Rounded,
			padding: SurfacePadding::ZERO,
			shadow: false,
			title: None,
		}
	}
}

/// Border variants for overlay surfaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SurfaceBorder {
	#[default]
	Rounded,
	Stripe,
}

/// Padding for overlay surface content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfacePadding {
	pub left: u16,
	pub right: u16,
	pub top: u16,
	pub bottom: u16,
}

impl SurfacePadding {
	pub const ZERO: Self = Self {
		left: 0,
		right: 0,
		top: 0,
		bottom: 0,
	};

	pub const fn horizontal(value: u16) -> Self {
		Self {
			left: value,
			right: value,
			top: 0,
			bottom: 0,
		}
	}
}
