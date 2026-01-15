//! Window manager for base and floating windows.

use std::collections::HashMap;

use xeno_tui::layout::Rect;

use super::types::{BaseWindow, FloatingStyle, FloatingWindow, Window, WindowId};
use crate::buffer::{BufferId, Layout};

/// Tracks all editor windows and their ordering.
pub struct WindowManager {
	next_id: u64,
	base: WindowId,
	windows: HashMap<WindowId, Window>,
	floating_order: Vec<WindowId>,
}

impl WindowManager {
	/// Creates a new window manager with a base window.
	pub fn new(base_layout: Layout, focused_buffer: BufferId) -> Self {
		let base_id = WindowId(1);
		let base_window = BaseWindow {
			layout: base_layout,
			focused_buffer,
		};
		let mut windows = HashMap::new();
		windows.insert(base_id, Window::Base(base_window));

		Self {
			next_id: base_id.0 + 1,
			base: base_id,
			windows,
			floating_order: Vec::new(),
		}
	}

	/// Returns the base window ID.
	pub fn base_id(&self) -> WindowId {
		self.base
	}

	/// Returns the base window.
	pub fn base_window(&self) -> &BaseWindow {
		match self
			.windows
			.get(&self.base)
			.expect("base window must exist")
		{
			Window::Base(base) => base,
			Window::Floating(_) => panic!("base window ID must reference base"),
		}
	}

	/// Returns the base window mutably.
	pub fn base_window_mut(&mut self) -> &mut BaseWindow {
		match self
			.windows
			.get_mut(&self.base)
			.expect("base window must exist")
		{
			Window::Base(base) => base,
			Window::Floating(_) => panic!("base window ID must reference base"),
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

	/// Iterates floating windows in z-order from bottom to top.
	pub fn floating_windows(&self) -> impl Iterator<Item = (WindowId, &FloatingWindow)> {
		self.floating_order
			.iter()
			.filter_map(|id| match self.windows.get(id) {
				Some(Window::Floating(window)) => Some((*id, window)),
				_ => None,
			})
	}

	/// Creates a new floating window and returns its ID.
	pub fn create_floating(
		&mut self,
		buffer: BufferId,
		rect: Rect,
		style: FloatingStyle,
	) -> WindowId {
		let id = WindowId(self.next_id);
		self.next_id += 1;

		let window = FloatingWindow::new(id, buffer, rect, style);
		self.windows.insert(id, Window::Floating(window));
		self.floating_order.push(id);
		id
	}

	/// Closes a floating window by ID.
	pub fn close_floating(&mut self, id: WindowId) {
		if matches!(self.windows.get(&id), Some(Window::Floating(_))) {
			self.windows.remove(&id);
			self.floating_order.retain(|window_id| *window_id != id);
		}
	}
}
