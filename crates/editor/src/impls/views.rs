//! Buffer access and viewport management.
//!
//! Provides convenient methods for accessing buffers. Delegates to [`ViewManager`].

use xeno_registry::options::option_keys as keys;

use super::{Editor, FocusTarget};
use crate::buffer::{Buffer, ViewId};
use crate::window::Window;

impl Editor {
	/// Returns a reference to the currently focused text buffer.
	#[inline]
	pub fn buffer(&self) -> &Buffer {
		self.focused_buffer()
	}

	/// Returns a mutable reference to the currently focused text buffer.
	#[inline]
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		self.focused_buffer_mut()
	}

	/// Returns the currently focused window.
	pub fn focused_window(&self) -> &Window {
		match &self.state.core.focus {
			FocusTarget::Buffer { window, .. } => self.state.core.windows.get(*window).expect("focused window must exist"),
			FocusTarget::Overlay { .. } => self.state.core.windows.get(self.state.core.windows.base_id()).expect("base window must exist"),
			FocusTarget::Panel(_) => self.state.core.windows.get(self.state.core.windows.base_id()).expect("base window must exist"),
		}
	}

	/// Returns a reference to the currently focused text buffer.
	#[inline]
	pub fn focused_buffer(&self) -> &Buffer {
		let buffer_id = self.focused_view();
		self.state.core.editor.buffers.get_buffer(buffer_id).expect("focused buffer must exist")
	}

	/// Returns a mutable reference to the currently focused text buffer.
	#[inline]
	pub fn focused_buffer_mut(&mut self) -> &mut Buffer {
		let buffer_id = self.focused_view();
		self.state.core.editor.buffers.get_buffer_mut(buffer_id).expect("focused buffer must exist")
	}

	/// Returns the currently focused view (buffer ID).
	pub fn focused_view(&self) -> ViewId {
		match &self.state.core.focus {
			FocusTarget::Buffer { buffer, .. } => *buffer,
			FocusTarget::Overlay { buffer } => *buffer,
			FocusTarget::Panel(_) => self.base_window().focused_buffer,
		}
	}

	/// Returns true if the focused view is a text buffer.
	pub fn is_text_focused(&self) -> bool {
		matches!(self.state.core.focus, FocusTarget::Buffer { .. })
	}

	/// Returns the ID of the focused text buffer.
	pub fn focused_buffer_id(&self) -> Option<ViewId> {
		Some(self.focused_view())
	}

	/// Returns all text buffer IDs.
	pub fn buffer_ids(&self) -> Vec<ViewId> {
		self.state.core.editor.buffers.buffer_ids().collect()
	}

	/// Returns a reference to a specific buffer by ID.
	pub fn get_buffer(&self, id: ViewId) -> Option<&Buffer> {
		self.state.core.editor.buffers.get_buffer(id)
	}

	/// Returns a mutable reference to a specific buffer by ID.
	pub fn get_buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer> {
		self.state.core.editor.buffers.get_buffer_mut(id)
	}

	/// Returns the number of open text buffers.
	pub fn buffer_count(&self) -> usize {
		self.state.core.editor.buffers.buffer_count()
	}

	/// Returns the tab width for a specific buffer.
	///
	/// Resolves through the option layers: buffer-local → language → global → default.
	/// This helper is useful when you need to pre-resolve the option before borrowing
	/// the buffer mutably.
	pub fn tab_width_for(&self, buffer_id: ViewId) -> usize {
		self.state
			.core
			.buffers
			.get_buffer(buffer_id)
			.map(|b| (b.option(keys::TAB_WIDTH, self) as usize).max(1))
			.unwrap_or(4)
	}

	/// Returns the tab width for the currently focused buffer.
	pub fn tab_width(&self) -> usize {
		(self.buffer().option(keys::TAB_WIDTH, self) as usize).max(1)
	}

	/// Returns the scroll-lines setting for a specific buffer.
	pub fn scroll_lines_for(&self, buffer_id: ViewId) -> usize {
		self.state
			.core
			.buffers
			.get_buffer(buffer_id)
			.map(|b| (b.option(keys::SCROLL_LINES, self) as usize).max(1))
			.unwrap_or(1)
	}

	/// Returns whether cursorline is enabled for a specific buffer.
	pub fn cursorline_for(&self, buffer_id: ViewId) -> bool {
		self.state
			.core
			.buffers
			.get_buffer(buffer_id)
			.map(|b| b.option(keys::CURSORLINE, self))
			.unwrap_or(true)
	}

	/// Returns the scroll margin for a specific buffer.
	pub fn scroll_margin_for(&self, buffer_id: ViewId) -> usize {
		self.state
			.core
			.buffers
			.get_buffer(buffer_id)
			.map(|b| b.option(keys::SCROLL_MARGIN, self) as usize)
			.unwrap_or(5)
	}

	/// Returns the screen area of a specific view.
	pub fn view_area(&self, view_id: ViewId) -> crate::geometry::Rect {
		if let Some(active) = self.state.ui.overlay_system.interaction().active()
			&& let Some(pane) = active.session.panes.iter().find(|pane| pane.buffer == view_id)
		{
			return pane.content_rect;
		}

		for (_, window) in self.state.core.windows.windows() {
			if window.buffer() == view_id && matches!(window, Window::Base(_)) {
				let doc_area = self.doc_area();
				for (v, area) in self.state.core.layout.compute_view_areas(&self.base_window().layout, doc_area) {
					if v == view_id {
						return area;
					}
				}
			}
		}
		self.doc_area()
	}
}
