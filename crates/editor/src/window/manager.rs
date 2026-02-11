//! Window manager for base window state. See [`layout::manager`](crate::layout::manager) for full windowing architecture.

use std::collections::HashMap;

use super::types::{BaseWindow, Window, WindowId};
use crate::buffer::{Layout, ViewId};

/// Tracks all editor windows and their ordering.
pub struct WindowManager {
	base: WindowId,
	windows: HashMap<WindowId, Window>,
}

impl WindowManager {
	/// Creates a new window manager with a base window.
	pub fn new(base_layout: Layout, focused_buffer: ViewId) -> Self {
		let base_id = WindowId(1);
		let base_window = BaseWindow {
			layout: base_layout,
			focused_buffer,
		};
		let mut windows = HashMap::new();
		windows.insert(base_id, Window::Base(base_window));

		Self { base: base_id, windows }
	}

	/// Returns the base window ID.
	pub fn base_id(&self) -> WindowId {
		self.base
	}

	/// Returns the base window.
	pub fn base_window(&self) -> &BaseWindow {
		match self.windows.get(&self.base).expect("base window must exist") {
			Window::Base(base) => base,
		}
	}

	/// Returns the base window mutably.
	pub fn base_window_mut(&mut self) -> &mut BaseWindow {
		match self.windows.get_mut(&self.base).expect("base window must exist") {
			Window::Base(base) => base,
		}
	}

	/// Returns a window by ID.
	pub fn get(&self, id: WindowId) -> Option<&Window> {
		self.windows.get(&id)
	}

	/// Returns a window by ID mutably.
	pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
		self.windows.get_mut(&id)
	}

	/// Iterates all windows.
	pub fn windows(&self) -> impl Iterator<Item = (WindowId, &Window)> {
		self.windows.iter().map(|(id, w)| (*id, w))
	}

	// Floating-window APIs were removed in favor of surface layers.
}
