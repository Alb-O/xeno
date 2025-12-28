//! Buffer and terminal view access.
//!
//! Provides convenient methods for accessing the focused view and navigating
//! between buffers and terminals. These delegate to [`BufferManager`].

use evildoer_manifest::SplitBuffer;

use super::Editor;
use crate::buffer::{Buffer, BufferId, BufferView, TerminalId};
use crate::terminal::TerminalBuffer;

impl Editor {
	/// Returns a reference to the currently focused text buffer.
	///
	/// Panics if the focused view is a terminal.
	#[inline]
	pub fn buffer(&self) -> &Buffer {
		self.buffers.focused_buffer()
	}

	/// Returns a mutable reference to the currently focused text buffer.
	///
	/// Panics if the focused view is a terminal.
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

	/// Returns true if the focused view is a terminal.
	pub fn is_terminal_focused(&self) -> bool {
		self.buffers.is_terminal_focused()
	}

	/// Returns the ID of the focused text buffer, if one is focused.
	pub fn focused_buffer_id(&self) -> Option<BufferId> {
		self.buffers.focused_buffer_id()
	}

	/// Returns the ID of the focused terminal, if one is focused.
	pub fn focused_terminal_id(&self) -> Option<TerminalId> {
		self.buffers.focused_terminal_id()
	}

	/// Returns all text buffer IDs.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.buffers.buffer_ids().collect()
	}

	/// Returns all terminal IDs.
	pub fn terminal_ids(&self) -> Vec<TerminalId> {
		self.buffers.terminal_ids().collect()
	}

	/// Returns a reference to a specific buffer by ID.
	pub fn get_buffer(&self, id: BufferId) -> Option<&Buffer> {
		self.buffers.get_buffer(id)
	}

	/// Returns a mutable reference to a specific buffer by ID.
	pub fn get_buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
		self.buffers.get_buffer_mut(id)
	}

	/// Returns a reference to a specific terminal by ID.
	pub fn get_terminal(&self, id: TerminalId) -> Option<&TerminalBuffer> {
		self.buffers.get_terminal(id)
	}

	/// Returns a mutable reference to a specific terminal by ID.
	pub fn get_terminal_mut(&mut self, id: TerminalId) -> Option<&mut TerminalBuffer> {
		self.buffers.get_terminal_mut(id)
	}

	/// Returns the number of open text buffers.
	pub fn buffer_count(&self) -> usize {
		self.buffers.buffer_count()
	}

	/// Returns the number of open terminals.
	pub fn terminal_count(&self) -> usize {
		self.buffers.terminal_count()
	}

	/// Returns the cursor style for the focused terminal, if any.
	pub fn focused_terminal_cursor_style(&self) -> Option<evildoer_manifest::SplitCursorStyle> {
		let terminal_id = self.focused_terminal_id()?;
		let terminal = self.get_terminal(terminal_id)?;
		terminal.cursor().map(|c| c.style)
	}
}
