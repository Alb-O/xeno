//! Editor context and capability traits for action result handling.
//!
//! This module provides the bridge between action results and editor state.
//! When an action returns an [`ActionResult`], the editor uses [`EditorContext`]
//! to apply the result.
//!
//! # Capability System
//!
//! The editor's capabilities are split into fine-grained traits:
//!
//! - **Required**: [`CursorAccess`], [`SelectionAccess`], [`TextAccess`], [`ModeAccess`], [`MessageAccess`]
//! - **Optional**: [`SearchAccess`], [`UndoAccess`], [`EditAccess`], [`BufferOpsAccess`], etc.
//!
//! [`EditorCapabilities`] combines the required traits and provides accessors
//! for optional capabilities. This allows actions to gracefully degrade when
//! certain features aren't available.
//!
//! [`ActionResult`]: crate::ActionResult

mod capabilities;
mod handlers;

pub use capabilities::*;
pub use handlers::*;
use ropey::RopeSlice;
use tome_base::range::CharIdx;
use tome_base::selection::Selection;

use crate::{Capability, CommandError, Mode};

/// Context for applying action results to editor state.
///
/// Wraps an [`EditorCapabilities`] implementor and provides convenient methods
/// for common operations. Used by the result dispatch system to translate
/// [`ActionResult`] variants into editor mutations.
///
/// # Capability Checking
///
/// Use [`check_capability`] or `require_*` methods to safely access
/// optional capabilities:
///
/// ```ignore
/// // Optional access - returns None if unavailable
/// if let Some(search) = ctx.search() {
///     search.search_next(false, false);
/// }
///
/// // Required access - returns error if unavailable
/// let edit = ctx.require_edit()?;
/// edit.execute_edit(&action, false);
/// ```
///
/// [`ActionResult`]: crate::ActionResult
/// [`check_capability`]: Self::check_capability
pub struct EditorContext<'a> {
	/// The capability provider (typically [`Editor`] from tome-api).
	///
	/// [`Editor`]: crate::Editor
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
			BufferOps => self.inner.buffer_ops().is_some(),
			FileOps => self.inner.file_ops().is_some(),
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
///
/// This trait combines required capability traits and provides optional accessors
/// for extended features. The base traits are always available:
///
/// - [`CursorAccess`] - Cursor position get/set
/// - [`SelectionAccess`] - Selection state management
/// - [`TextAccess`] - Read-only document access
/// - [`ModeAccess`] - Editor mode (Normal, Insert, etc.)
/// - [`MessageAccess`] - Status messages and notifications
///
/// Optional capabilities default to `None` but can be overridden by implementors.
/// Check availability with [`EditorContext::check_capability`] before use.
///
/// # Implementing
///
/// Implement the required traits, then override the optional methods for
/// any additional capabilities your editor supports:
///
/// ```ignore
/// impl EditorCapabilities for MyEditor {
///     fn search(&mut self) -> Option<&mut dyn SearchAccess> {
///         Some(self)  // MyEditor also implements SearchAccess
///     }
///     // ... other overrides
/// }
/// ```
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

	/// Access to buffer/split management operations (optional).
	fn buffer_ops(&mut self) -> Option<&mut dyn BufferOpsAccess> {
		None
	}

	/// Access to file operations (optional).
	fn file_ops(&mut self) -> Option<&mut dyn FileOpsAccess> {
		None
	}
}
