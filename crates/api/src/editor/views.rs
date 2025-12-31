//! Buffer and panel view access.
//!
//! Provides convenient methods for accessing the focused view and navigating
//! between buffers and panels. These delegate to [`BufferManager`] and [`PanelRegistry`].

use evildoer_manifest::{PANELS, PanelDef, PanelId};

use super::Editor;
use crate::buffer::{Buffer, BufferId, BufferView};

impl Editor {
	/// Returns a reference to the currently focused text buffer.
	///
	/// Panics if the focused view is not a text buffer.
	#[inline]
	pub fn buffer(&self) -> &Buffer {
		self.buffers.focused_buffer()
	}

	/// Returns a mutable reference to the currently focused text buffer.
	///
	/// Panics if the focused view is not a text buffer.
	#[inline]
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		self.buffers.focused_buffer_mut()
	}

	/// Returns the currently focused view.
	pub fn focused_view(&self) -> BufferView {
		self.buffers.focused_view()
	}

	/// Returns true if the focused view is a text buffer.
	pub fn is_text_focused(&self) -> bool {
		self.buffers.is_text_focused()
	}

	/// Returns true if the focused view is a panel.
	pub fn is_panel_focused(&self) -> bool {
		matches!(self.focused_view(), BufferView::Panel(_))
	}

	/// Returns true if the focused view captures panel input.
	pub fn is_terminal_focused(&self) -> bool {
		self.focused_panel_def()
			.is_some_and(|panel| panel.captures_input)
	}

	/// Returns true if the focused view is a non-capturing panel.
	pub fn is_debug_focused(&self) -> bool {
		self.focused_panel_def()
			.is_some_and(|panel| !panel.captures_input)
	}

	/// Returns the ID of the focused text buffer, if one is focused.
	pub fn focused_buffer_id(&self) -> Option<BufferId> {
		self.buffers.focused_buffer_id()
	}

	/// Returns the ID of the focused panel, if one is focused.
	pub fn focused_panel_id(&self) -> Option<PanelId> {
		self.focused_view().as_panel()
	}

	/// Returns the panel definition for the focused panel, if any.
	pub fn focused_panel_def(&self) -> Option<&'static PanelDef> {
		let panel_id = self.focused_panel_id()?;
		PANELS.get(panel_id.kind as usize)
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

	/// Returns the cursor style for the focused panel, if any.
	pub fn focused_panel_cursor_style(&self) -> Option<evildoer_manifest::SplitCursorStyle> {
		let panel_id = self.focused_panel_id()?;
		let panel = self.panels.get(panel_id)?;
		panel.cursor().map(|c| c.style)
	}
}
