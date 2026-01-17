//! Core editing state extracted from Editor.
//!
//! [`EditorCore`] contains the essential state for text editing operations:
//! buffers, workspace session state, and undo history. UI, layout, and
//! presentation concerns remain in [`Editor`].
//!
//! This separation enables:
//! - Independent testing of editing logic
//! - Cleaner borrow checker relationships (borrow core separately from UI)
//! - Capability trait implementations focused on editing concerns
//!
//! [`Editor`]: super::Editor

use xeno_primitives::Selection;
use xeno_primitives::range::CharIdx;
use xeno_registry::{CommandQueueAccess, CursorAccess, MacroAccess, SelectionAccess};

use crate::buffer::{Buffer, ViewId};
use crate::types::{UndoManager, Workspace};
use crate::view_manager::ViewManager;

/// Core editing state: buffers, workspace, undo history.
///
/// Contains the essential state for text editing operations without
/// UI, layout, or presentation concerns. Capability traits that only
/// need editing state can be implemented here.
///
/// # Structure
///
/// ```text
/// EditorCore
/// ├── buffers: ViewManager       // Text buffer storage and focus tracking
/// ├── workspace: Workspace       // Session state (registers, jumps, macros)
/// └── undo_manager: UndoManager  // Undo/redo grouping stacks
/// ```
pub struct EditorCore {
	/// Buffer and document storage.
	///
	/// Manages text buffers, tracks focused view, and generates unique IDs.
	pub buffers: ViewManager,

	/// Session state persisting across buffer switches.
	///
	/// Contains registers (yank buffer), jump list, macro state, and command queue.
	pub workspace: Workspace,

	/// Editor-level undo manager.
	///
	/// Manages undo/redo grouping stacks. Each entry captures view state
	/// (cursor, selection, scroll) for all affected buffers at the time of
	/// the edit. Document state is stored separately in each document's history.
	pub undo_manager: UndoManager,
}

impl EditorCore {
	/// Creates a new EditorCore with the given components.
	pub fn new(buffers: ViewManager, workspace: Workspace, undo_manager: UndoManager) -> Self {
		Self {
			buffers,
			workspace,
			undo_manager,
		}
	}

	/// Returns the focused view (buffer ID).
	#[inline]
	pub fn focused_view(&self) -> ViewId {
		self.buffers.focused_view()
	}

	/// Returns the focused buffer.
	///
	/// # Panics
	///
	/// Panics if the focused buffer doesn't exist.
	#[inline]
	pub fn buffer(&self) -> &Buffer {
		self.buffers.focused_buffer()
	}

	/// Returns the focused buffer mutably.
	///
	/// # Panics
	///
	/// Panics if the focused buffer doesn't exist.
	#[inline]
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		self.buffers.focused_buffer_mut()
	}

	/// Returns a buffer by ID.
	pub fn get_buffer(&self, id: ViewId) -> Option<&Buffer> {
		self.buffers.get_buffer(id)
	}

	/// Returns a buffer mutably by ID.
	pub fn get_buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer> {
		self.buffers.get_buffer_mut(id)
	}
}

impl CursorAccess for EditorCore {
	fn cursor(&self) -> CharIdx {
		self.buffer().cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		let buffer = self.buffer();
		Some((buffer.cursor_line(), buffer.cursor_col()))
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		self.buffer_mut().set_cursor(pos);
	}
}

impl SelectionAccess for EditorCore {
	fn selection(&self) -> &Selection {
		&self.buffer().selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.buffer_mut().selection
	}

	fn set_selection(&mut self, sel: Selection) {
		self.buffer_mut().set_selection(sel);
	}
}

impl MacroAccess for EditorCore {
	fn record(&mut self) {
		self.workspace.macro_state.start_recording('q');
	}

	fn stop_recording(&mut self) {
		self.workspace.macro_state.stop_recording();
	}

	fn play(&mut self) {
		// TODO: Requires event loop integration
	}

	fn is_recording(&self) -> bool {
		self.workspace.macro_state.is_recording()
	}
}

impl CommandQueueAccess for EditorCore {
	fn queue_command(&mut self, name: &'static str, args: Vec<String>) {
		self.workspace.command_queue.push(name, args);
	}
}
