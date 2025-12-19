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

	pub fn error(&mut self, msg: &str) {
		self.inner.show_error(msg);
	}

	pub fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		self.inner.search()
	}

	pub fn require_search(&mut self) -> Result<&mut dyn SearchAccess, crate::ext::CommandError> {
		self.inner.search().ok_or_else(|| {
			crate::ext::CommandError::Failed("Search capability not available".to_string())
		})
	}

	pub fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		self.inner.undo()
	}

	pub fn require_undo(&mut self) -> Result<&mut dyn UndoAccess, crate::ext::CommandError> {
		self.inner.undo().ok_or_else(|| {
			crate::ext::CommandError::Failed("Undo capability not available".to_string())
		})
	}

	pub fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		self.inner.edit()
	}

	pub fn require_edit(&mut self) -> Result<&mut dyn EditAccess, crate::ext::CommandError> {
		self.inner.edit().ok_or_else(|| {
			crate::ext::CommandError::Failed("Edit capability not available".to_string())
		})
	}

	pub fn selection_ops(&mut self) -> Option<&mut dyn SelectionOpsAccess> {
		self.inner.selection_ops()
	}

	pub fn require_selection_ops(
		&mut self,
	) -> Result<&mut dyn SelectionOpsAccess, crate::ext::CommandError> {
		self.inner.selection_ops().ok_or_else(|| {
			crate::ext::CommandError::Failed("Selection operations not available".to_string())
		})
	}
}

/// Core capabilities that all editors must provide.
pub trait EditorCapabilities:
	CursorAccess + SelectionAccess + TextAccess + ModeAccess + MessageAccess
{
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
