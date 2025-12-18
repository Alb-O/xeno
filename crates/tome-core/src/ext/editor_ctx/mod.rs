//! Editor context and capability traits for action result handling.
//!
//! This module provides a capability-based abstraction for editor operations.
//! Instead of one monolithic trait, we define fine-grained capabilities that
//! handlers can request access to.
//!
//! # Architecture
//!
//! - `EditorContext`: Central context passed to result handlers
//! - Capability traits: `CursorAccess`, `SelectionAccess`, `MessageAccess`, etc.
//! - `tome-term` implements these traits on its Editor struct
//! - Handlers request only the capabilities they need

mod capabilities;
mod handlers;
mod result_handlers;

pub use capabilities::*;
pub use handlers::*;
use ropey::RopeSlice;

use crate::Mode;
use crate::selection::Selection;

/// Context passed to action result handlers.
///
/// Provides capability-based access to editor state. Handlers downcast
/// to the specific capability traits they need.
pub struct EditorContext<'a> {
	/// The capability provider (typically Editor from tome-term).
	inner: &'a mut dyn EditorCapabilities,
}

impl<'a> EditorContext<'a> {
	pub fn new(inner: &'a mut dyn EditorCapabilities) -> Self {
		Self { inner }
	}

	pub fn cursor(&self) -> usize {
		self.inner.cursor()
	}

	pub fn set_cursor(&mut self, pos: usize) {
		self.inner.set_cursor(pos);
	}

	pub fn selection(&self) -> &Selection {
		self.inner.selection()
	}

	pub fn set_selection(&mut self, sel: Selection) {
		self.inner.set_selection(sel);
	}

	pub fn text(&self) -> RopeSlice<'_> {
		self.inner.text()
	}

	pub fn set_mode(&mut self, mode: Mode) {
		self.inner.set_mode(mode);
	}

	pub fn message(&mut self, msg: &str) {
		self.inner.show_message(msg);
	}

	pub fn scratch(&mut self) -> Option<&mut dyn ScratchAccess> {
		self.inner.scratch()
	}

	pub fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		self.inner.search()
	}

	pub fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		self.inner.undo()
	}

	pub fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		self.inner.edit()
	}

	pub fn selection_ops(&mut self) -> Option<&mut dyn SelectionOpsAccess> {
		self.inner.selection_ops()
	}
}

/// Core capabilities that all editors must provide.
pub trait EditorCapabilities:
	CursorAccess + SelectionAccess + TextAccess + ModeAccess + MessageAccess
{
	/// Access to scratch buffer operations (optional).
	fn scratch(&mut self) -> Option<&mut dyn ScratchAccess> {
		None
	}

	/// Access to search operations (optional).
	fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		None
	}

	/// Access to undo/redo operations (optional).
	fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		None
	}

	/// Access to edit operations (optional).
	fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		None
	}

	/// Access to selection manipulation operations (optional).
	fn selection_ops(&mut self) -> Option<&mut dyn SelectionOpsAccess> {
		None
	}
}
