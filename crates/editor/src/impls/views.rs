//! Buffer access and viewport management.
//!
//! Provides convenient methods for accessing buffers. Delegates to [`BufferManager`].

use xeno_registry::options::keys;

use super::{Editor, FocusTarget};
use crate::buffer::{Buffer, BufferId, BufferView};
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
		match &self.focus {
			FocusTarget::Buffer { window, .. } => self
				.windows
				.get(*window)
				.expect("focused window must exist"),
			FocusTarget::Panel(_) => self
				.windows
				.get(self.windows.base_id())
				.expect("base window must exist"),
		}
	}

	/// Returns a reference to the currently focused text buffer.
	#[inline]
	pub fn focused_buffer(&self) -> &Buffer {
		let buffer_id = self.focused_view();
		self.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
	}

	/// Returns a mutable reference to the currently focused text buffer.
	#[inline]
	pub fn focused_buffer_mut(&mut self) -> &mut Buffer {
		let buffer_id = self.focused_view();
		self.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist")
	}

	/// Returns the currently focused view (buffer ID).
	pub fn focused_view(&self) -> BufferView {
		match &self.focus {
			FocusTarget::Buffer { buffer, .. } => *buffer,
			FocusTarget::Panel(_) => self.base_window().focused_buffer,
		}
	}

	/// Returns true if the focused view is a text buffer.
	pub fn is_text_focused(&self) -> bool {
		matches!(self.focus, FocusTarget::Buffer { .. })
	}

	/// Returns the ID of the focused text buffer.
	pub fn focused_buffer_id(&self) -> Option<BufferId> {
		Some(self.focused_view())
	}

	/// Returns all text buffer IDs.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.buffers.buffer_ids().collect()
	}

	/// Returns a reference to a specific buffer by ID.
	pub fn get_buffer(&self, id: BufferId) -> Option<&Buffer> {
		self.buffers.get_buffer(id)
	}

	/// Returns a mutable reference to a specific buffer by ID.
	pub fn get_buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
		self.buffers.get_buffer_mut(id)
	}

	/// Returns the number of open text buffers.
	pub fn buffer_count(&self) -> usize {
		self.buffers.buffer_count()
	}

	/// Returns the tab width for a specific buffer.
	///
	/// Resolves through the option layers: buffer-local → language → global → default.
	/// This helper is useful when you need to pre-resolve the option before borrowing
	/// the buffer mutably.
	pub fn tab_width_for(&self, buffer_id: BufferId) -> usize {
		self.buffers
			.get_buffer(buffer_id)
			.map(|b| (b.option(keys::TAB_WIDTH, self) as usize).max(1))
			.unwrap_or(4)
	}

	/// Returns the tab width for the currently focused buffer.
	pub fn tab_width(&self) -> usize {
		(self.buffer().option(keys::TAB_WIDTH, self) as usize).max(1)
	}

	/// Returns whether cursorline is enabled for a specific buffer.
	pub fn cursorline_for(&self, buffer_id: BufferId) -> bool {
		self.buffers
			.get_buffer(buffer_id)
			.map(|b| b.option(keys::CURSORLINE, self))
			.unwrap_or(true)
	}

	/// Returns the scroll margin for a specific buffer.
	pub fn scroll_margin_for(&self, buffer_id: BufferId) -> usize {
		self.buffers
			.get_buffer(buffer_id)
			.map(|b| b.option(keys::SCROLL_MARGIN, self) as usize)
			.unwrap_or(5)
	}
}
