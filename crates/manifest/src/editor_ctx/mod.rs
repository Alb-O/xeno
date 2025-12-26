//! Editor context and capability traits for action result handling.

mod capabilities;
mod handlers;

pub use capabilities::*;
pub use handlers::*;
use ropey::RopeSlice;
use tome_base::range::CharIdx;
use tome_base::selection::Selection;

use crate::{Capability, CommandError, Mode};

/// Context passed to action result handlers.
pub struct EditorContext<'a> {
	/// The capability provider (typically Editor from tome-term).
	inner: &'a mut dyn EditorCapabilities,
}

impl<'a> MessageAccess for EditorContext<'a> {
	fn notify(&mut self, type_id: &str, msg: &str) {
		self.inner.notify(type_id, msg);
	}

	fn clear_message(&mut self) {
		self.inner.clear_message();
	}
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

	pub fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		self.inner.search()
	}

	pub fn require_search(&mut self) -> Result<&mut dyn SearchAccess, CommandError> {
		self.inner
			.search()
			.ok_or(CommandError::MissingCapability(Capability::Search))
	}

	pub fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		self.inner.undo()
	}

	pub fn require_undo(&mut self) -> Result<&mut dyn UndoAccess, CommandError> {
		self.inner
			.undo()
			.ok_or(CommandError::MissingCapability(Capability::Undo))
	}

	pub fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		self.inner.edit()
	}

	pub fn require_edit(&mut self) -> Result<&mut dyn EditAccess, CommandError> {
		self.inner
			.edit()
			.ok_or(CommandError::MissingCapability(Capability::Edit))
	}

	pub fn selection_ops(&mut self) -> Option<&mut dyn SelectionOpsAccess> {
		self.inner.selection_ops()
	}

	pub fn require_selection_ops(&mut self) -> Result<&mut dyn SelectionOpsAccess, CommandError> {
		self.inner
			.selection_ops()
			.ok_or(CommandError::MissingCapability(Capability::SelectionOps))
	}

	pub fn buffer_ops(&mut self) -> Option<&mut dyn BufferOpsAccess> {
		self.inner.buffer_ops()
	}

	pub fn require_buffer_ops(&mut self) -> Result<&mut dyn BufferOpsAccess, CommandError> {
		self.inner
			.buffer_ops()
			.ok_or(CommandError::MissingCapability(Capability::BufferOps))
	}

	pub fn check_capability(&mut self, cap: Capability) -> bool {
		use Capability::*;
		match cap {
			Text | Cursor | Selection | Mode | Messaging => true, // Basic ones are required by trait
			Edit => self.inner.edit().is_some(),
			Search => self.inner.search().is_some(),
			Undo => self.inner.undo().is_some(),
			SelectionOps => self.inner.selection_ops().is_some(),
			BufferOps => self.inner.buffer_ops().is_some(),
			Jump => false,      // Not yet implemented in traits
			Macro => false,     // Not yet implemented in traits
			Transform => false, // Not yet implemented in traits
			FileOps => false,   // Not yet implemented in traits
		}
	}

	pub fn check_all_capabilities(&mut self, caps: &[Capability]) -> Result<(), CommandError> {
		for &cap in caps {
			if !self.check_capability(cap) {
				return Err(CommandError::MissingCapability(cap));
			}
		}
		Ok(())
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

	/// Access to buffer/split management operations (optional).
	fn buffer_ops(&mut self) -> Option<&mut dyn BufferOpsAccess> {
		None
	}
}
