//! Editor context and capability traits for action result handling.

mod capabilities;
mod handlers;
mod result_handlers;

pub use capabilities::*;
pub use handlers::*;
use ropey::RopeSlice;

use crate::range::{CharIdx, Range};
use crate::Mode;
use crate::selection::Selection;

/// Context passed to action result handlers.
pub struct EditorContext<'a> {
	/// The capability provider (typically Editor from tome-term).
	inner: &'a mut dyn EditorCapabilities,
}

impl<'a> EditorContext<'a> {
	pub fn new(inner: &'a mut dyn EditorCapabilities) -> Self {
		Self { inner }
	}

	pub fn cursor(&self) -> CharIdx {
		self.inner.cursor()
	}

	pub fn set_cursor(&mut self, pos: CharIdx) {
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

	/// Generic helper to require a specific capability.
	pub fn require_capability<T: ?Sized + 'static>(
		&mut self,
		name: &str,
		accessor: impl FnOnce(&mut dyn EditorCapabilities) -> Option<&mut T>,
	) -> Result<&mut T, crate::ext::CommandError> {
		accessor(self.inner).ok_or_else(|| {
			crate::ext::CommandError::Failed(format!("{} capability not available", name))
		})
	}

	pub fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		self.inner.search()
	}

	pub fn require_search(&mut self) -> Result<&mut dyn SearchAccess, crate::ext::CommandError> {
		self.require_capability("Search", |i| i.search())
	}

	pub fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		self.inner.undo()
	}

	pub fn require_undo(&mut self) -> Result<&mut dyn UndoAccess, crate::ext::CommandError> {
		self.require_capability("Undo", |i| i.undo())
	}

	pub fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		self.inner.edit()
	}

	pub fn require_edit(&mut self) -> Result<&mut dyn EditAccess, crate::ext::CommandError> {
		self.require_capability("Edit", |i| i.edit())
	}

	pub fn selection_ops(&mut self) -> Option<&mut dyn SelectionOpsAccess> {
		self.inner.selection_ops()
	}

	pub fn require_selection_ops(
		&mut self,
	) -> Result<&mut dyn SelectionOpsAccess, crate::ext::CommandError> {
		self.require_capability("Selection operations", |i| i.selection_ops())
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
