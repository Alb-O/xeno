//! Buffer storage, ID generation, and focus tracking.
//!
//! [`ViewManager`] centralizes ownership of text buffers.
//! Focus state is mirrored from the [`Editor`] for compatibility.

use std::collections::HashMap;
use std::path::PathBuf;

use xeno_runtime_language::LanguageLoader;

use crate::buffer::{Buffer, ViewId};

/// Owns text buffers, tracks focus, and generates unique IDs.
pub struct ViewManager {
	/// Map of buffer IDs to their buffer instances.
	buffers: HashMap<ViewId, Buffer>,
	/// Counter for generating unique buffer IDs.
	next_buffer_id: u64,
	/// Currently focused view (buffer ID), mirrored from the editor focus.
	focused_view: ViewId,
}

impl ViewManager {
	/// Creates a manager with an initial buffer (ID 1) as the focused view.
	pub fn new(content: String, path: Option<PathBuf>, language_loader: &LanguageLoader) -> Self {
		let buffer_id = ViewId(1);
		let buffer = Buffer::new(buffer_id, content, path);
		buffer.init_syntax(language_loader);

		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		Self {
			buffers,
			next_buffer_id: 2,
			focused_view: buffer_id,
		}
	}

	/// Creates a manager with an existing buffer.
	pub fn with_buffer(buffer: Buffer) -> Self {
		let buffer_id = buffer.id;
		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		Self {
			buffers,
			next_buffer_id: buffer_id.0 + 1,
			focused_view: buffer_id,
		}
	}

	/// Creates a new buffer with syntax highlighting. Does not change focus.
	pub fn create_buffer(
		&mut self,
		content: String,
		path: Option<PathBuf>,
		language_loader: &LanguageLoader,
		window_width: Option<u16>,
	) -> ViewId {
		let buffer_id = ViewId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let mut buffer = Buffer::new(buffer_id, content, path);
		buffer.init_syntax(language_loader);

		if let Some(width) = window_width {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		self.buffers.insert(buffer_id, buffer);
		buffer_id
	}

	/// Creates an empty scratch buffer without syntax highlighting.
	///
	/// Used for temporary input buffers like command palette.
	pub fn create_scratch(&mut self) -> ViewId {
		let buffer_id = ViewId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let buffer = Buffer::new(buffer_id, String::new(), None);
		self.buffers.insert(buffer_id, buffer);
		buffer_id
	}

	/// Creates a new buffer that shares the same document as the focused buffer.
	///
	/// The new buffer has independent cursor/selection/scroll state but
	/// edits in either buffer affect both (they share the same Document).
	pub fn clone_focused_buffer_for_split(&mut self) -> ViewId {
		let new_id = ViewId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let new_buffer = self.focused_buffer().clone_for_split(new_id);
		self.buffers.insert(new_id, new_buffer);
		new_id
	}

	/// Removes a buffer. Does not update focus.
	pub fn remove_buffer(&mut self, id: ViewId) -> Option<Buffer> {
		self.buffers.remove(&id)
	}

	/// Returns the currently focused view (buffer ID).
	pub fn focused_view(&self) -> ViewId {
		self.focused_view
	}

	/// Sets the focused view. Returns true if the view exists.
	///
	/// This should be driven by the editor focus model.
	pub fn set_focused_view(&mut self, view: ViewId) -> bool {
		if self.buffers.contains_key(&view) {
			self.focused_view = view;
			true
		} else {
			false
		}
	}

	/// Returns true if the focused view is a text buffer.
	///
	/// Always returns true since all views are now text buffers.
	pub fn is_text_focused(&self) -> bool {
		true
	}

	/// Returns the ID of the focused text buffer.
	pub fn focused_buffer_id(&self) -> Option<ViewId> {
		Some(self.focused_view)
	}

	/// Returns the focused text buffer.
	///
	/// # Panics
	///
	/// Panics if the focused buffer doesn't exist.
	#[inline]
	pub fn focused_buffer(&self) -> &Buffer {
		self.buffers
			.get(&self.focused_view)
			.expect("focused buffer must exist")
	}

	/// Returns the focused text buffer mutably.
	///
	/// # Panics
	///
	/// Panics if the focused buffer doesn't exist.
	#[inline]
	pub fn focused_buffer_mut(&mut self) -> &mut Buffer {
		self.buffers
			.get_mut(&self.focused_view)
			.expect("focused buffer must exist")
	}

	/// Returns a buffer by ID.
	pub fn get_buffer(&self, id: ViewId) -> Option<&Buffer> {
		self.buffers.get(&id)
	}

	/// Returns a buffer mutably by ID.
	pub fn get_buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer> {
		self.buffers.get_mut(&id)
	}

	/// Returns an iterator over all buffer IDs.
	pub fn buffer_ids(&self) -> impl Iterator<Item = ViewId> + '_ {
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

	/// Finds a buffer by its file path.
	///
	/// Returns the first buffer that has a matching path. Note that multiple
	/// buffers may share the same document (via splits), so this returns
	/// just one of them.
	pub fn find_by_path(&self, path: &std::path::Path) -> Option<ViewId> {
		self.buffers
			.values()
			.find(|b| b.path().as_deref() == Some(path))
			.map(|b| b.id)
	}
}
