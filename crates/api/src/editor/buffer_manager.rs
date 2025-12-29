//! Buffer and terminal storage, ID generation, and focus tracking.
//!
//! [`BufferManager`] centralizes ownership of text buffers and terminals,
//! providing a single source of truth for what's open and what's focused.
//! Layout and hook emission remain in [`Editor`](super::Editor).

use std::collections::HashMap;
use std::path::PathBuf;

use evildoer_language::LanguageLoader;
use evildoer_manifest::SplitBuffer;

use crate::buffer::{Buffer, BufferId, BufferView, TerminalId};
use crate::terminal::TerminalBuffer;

/// Owns text buffers and terminals, tracks focus, and generates unique IDs.
pub struct BufferManager {
	buffers: HashMap<BufferId, Buffer>,
	terminals: HashMap<TerminalId, TerminalBuffer>,
	next_buffer_id: u64,
	next_terminal_id: u64,
	focused_view: BufferView,
}

impl BufferManager {
	/// Creates a manager with an initial buffer (ID 1) as the focused view.
	pub fn new(content: String, path: Option<PathBuf>, language_loader: &LanguageLoader) -> Self {
		let buffer_id = BufferId(1);
		let buffer = Buffer::new(buffer_id, content, path);
		buffer.init_syntax(language_loader);

		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		Self {
			buffers,
			terminals: HashMap::new(),
			next_buffer_id: 2,
			next_terminal_id: 1,
			focused_view: BufferView::Text(buffer_id),
		}
	}

	/// Creates a manager with an existing buffer.
	pub fn with_buffer(buffer: Buffer) -> Self {
		let buffer_id = buffer.id;
		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		Self {
			buffers,
			terminals: HashMap::new(),
			next_buffer_id: buffer_id.0 + 1,
			next_terminal_id: 1,
			focused_view: BufferView::Text(buffer_id),
		}
	}

	/// Creates a new buffer with syntax highlighting. Does not change focus.
	pub fn create_buffer(
		&mut self,
		content: String,
		path: Option<PathBuf>,
		language_loader: &LanguageLoader,
		window_width: Option<u16>,
	) -> BufferId {
		let buffer_id = BufferId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let mut buffer = Buffer::new(buffer_id, content, path);
		buffer.init_syntax(language_loader);

		if let Some(width) = window_width {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		self.buffers.insert(buffer_id, buffer);
		buffer_id
	}

	/// Creates a new terminal. Does not change focus.
	pub fn create_terminal(&mut self) -> TerminalId {
		let terminal_id = TerminalId(self.next_terminal_id);
		self.next_terminal_id += 1;

		let mut terminal = TerminalBuffer::new();
		terminal.on_open();
		self.terminals.insert(terminal_id, terminal);
		terminal_id
	}

	/// Creates a new buffer that shares the same document as the focused buffer.
	///
	/// The new buffer has independent cursor/selection/scroll state but
	/// edits in either buffer affect both (they share the same Document).
	///
	/// # Panics
	///
	/// Panics if the focused view is a terminal (not a text buffer).
	pub fn clone_focused_buffer_for_split(&mut self) -> BufferId {
		let new_id = BufferId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let new_buffer = self.focused_buffer().clone_for_split(new_id);
		self.buffers.insert(new_id, new_buffer);
		new_id
	}

	/// Removes a buffer. Does not update focus.
	pub fn remove_buffer(&mut self, id: BufferId) -> Option<Buffer> {
		self.buffers.remove(&id)
	}

	/// Removes a terminal. Does not update focus.
	pub fn remove_terminal(&mut self, id: TerminalId) -> Option<TerminalBuffer> {
		self.terminals.remove(&id)
	}

	/// Returns the currently focused view.
	pub fn focused_view(&self) -> BufferView {
		self.focused_view
	}

	/// Sets the focused view. Returns true if the view exists.
	pub fn set_focused_view(&mut self, view: BufferView) -> bool {
		let exists = match view {
			BufferView::Text(id) => self.buffers.contains_key(&id),
			BufferView::Terminal(id) => self.terminals.contains_key(&id),
		};
		if exists {
			self.focused_view = view;
		}
		exists
	}

	/// Returns true if the focused view is a text buffer.
	pub fn is_text_focused(&self) -> bool {
		self.focused_view.is_text()
	}

	/// Returns true if the focused view is a terminal.
	pub fn is_terminal_focused(&self) -> bool {
		self.focused_view.is_terminal()
	}

	/// Returns the ID of the focused text buffer, if one is focused.
	pub fn focused_buffer_id(&self) -> Option<BufferId> {
		self.focused_view.as_text()
	}

	/// Returns the ID of the focused terminal, if one is focused.
	pub fn focused_terminal_id(&self) -> Option<TerminalId> {
		self.focused_view.as_terminal()
	}

	/// Returns the focused text buffer.
	///
	/// # Panics
	///
	/// Panics if the focused view is a terminal.
	#[inline]
	pub fn focused_buffer(&self) -> &Buffer {
		match self.focused_view {
			BufferView::Text(id) => self.buffers.get(&id).expect("focused buffer must exist"),
			BufferView::Terminal(_) => panic!("focused view is a terminal, not a text buffer"),
		}
	}

	/// Returns the focused text buffer mutably.
	///
	/// # Panics
	///
	/// Panics if the focused view is a terminal.
	#[inline]
	pub fn focused_buffer_mut(&mut self) -> &mut Buffer {
		match self.focused_view {
			BufferView::Text(id) => self
				.buffers
				.get_mut(&id)
				.expect("focused buffer must exist"),
			BufferView::Terminal(_) => panic!("focused view is a terminal, not a text buffer"),
		}
	}

	/// Returns a buffer by ID.
	pub fn get_buffer(&self, id: BufferId) -> Option<&Buffer> {
		self.buffers.get(&id)
	}

	/// Returns a buffer mutably by ID.
	pub fn get_buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
		self.buffers.get_mut(&id)
	}

	/// Returns an iterator over all buffer IDs.
	pub fn buffer_ids(&self) -> impl Iterator<Item = BufferId> + '_ {
		self.buffers.keys().copied()
	}

	/// Returns the number of open text buffers.
	pub fn buffer_count(&self) -> usize {
		self.buffers.len()
	}

	/// Returns an iterator over all buffers.
	pub fn buffers(&self) -> impl Iterator<Item = &Buffer> {
		self.buffers.values()
	}

	/// Returns a mutable iterator over all buffers.
	pub fn buffers_mut(&mut self) -> impl Iterator<Item = &mut Buffer> {
		self.buffers.values_mut()
	}

	/// Returns a terminal by ID.
	pub fn get_terminal(&self, id: TerminalId) -> Option<&TerminalBuffer> {
		self.terminals.get(&id)
	}

	/// Returns a terminal mutably by ID.
	pub fn get_terminal_mut(&mut self, id: TerminalId) -> Option<&mut TerminalBuffer> {
		self.terminals.get_mut(&id)
	}

	/// Returns an iterator over all terminal IDs.
	pub fn terminal_ids(&self) -> impl Iterator<Item = TerminalId> + '_ {
		self.terminals.keys().copied()
	}

	/// Returns the number of open terminals.
	pub fn terminal_count(&self) -> usize {
		self.terminals.len()
	}

	/// Returns an iterator over all terminals.
	pub fn terminals(&self) -> impl Iterator<Item = &TerminalBuffer> {
		self.terminals.values()
	}

	/// Returns a mutable iterator over all terminals.
	pub fn terminals_mut(&mut self) -> impl Iterator<Item = &mut TerminalBuffer> {
		self.terminals.values_mut()
	}
}
