//! Context provided to actions during execution.
//!
//! # Purpose
//!
//! [`EditorContext`] provides controlled access to the editor's state and capabilities.
//! This allows actions to be written against a stable interface rather than internal
//! structures, and enables dependency injection for testing.
//!
//! # Design
//!
//! The context is a thin wrapper around a dyn trait object [`EditorCapabilities`].
//! This separates the interface (what actions can do) from the implementation
//! (how the editor does it).
//!
//! # Capabilities
//!
//! Capabilities are split into fine-grained traits (e.g., [`CursorAccess`], [`SearchAccess`]).
//! This allows actions to declare exactly what they need, and enables graceful
//! degradation when capabilities are missing (e.g., in a headless environment).
//!
//! See [`crate::actions::editor_ctx`] module for the full list of available traits.
//!
//! [`EditorContext`]: EditorContext
//! [`EditorCapabilities`]: EditorCapabilities
//! [`CursorAccess`]: capabilities::CursorAccess
//! [`SearchAccess`]: capabilities::SearchAccess

mod capabilities;
mod handlers;

pub use capabilities::{
	CursorAccess, DeferredInvocationAccess, EditAccess, EditorOps, FileOpsAccess, FocusOps, JumpAccess, MacroAccess, ModeAccess, MotionAccess,
	MotionDispatchAccess, NotificationAccess, OptionAccess, OverlayAccess, OverlayCloseReason, OverlayRequest, PaletteAccess, SearchAccess, SelectionAccess,
	SplitError, SplitOps, TextAccess, ThemeAccess, UndoAccess, ViewportAccess,
};
pub use handlers::{HandleOutcome, ResultHandler};
use xeno_primitives::range::CharIdx;
use xeno_primitives::selection::Selection;

pub use super::result::ResultHandlerRegistry;
use crate::actions::{Capability, CommandError, Mode};

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
/// [`ActionResult`]: crate::actions::ActionResult
/// [`check_capability`]: Self::check_capability
pub struct EditorContext<'a> {
	/// The capability provider (typically `EditorCaps` from xeno-editor).
	inner: &'a mut dyn EditorCapabilities,
}

impl<'a> NotificationAccess for EditorContext<'a> {
	fn emit(&mut self, notification: crate::notifications::Notification) {
		self.inner.emit(notification);
	}

	fn clear_notifications(&mut self) {
		self.inner.clear_notifications();
	}
}

impl<'a> EditorContext<'a> {
	/// Creates a new editor context wrapping the given capabilities.
	pub fn new(inner: &'a mut dyn EditorCapabilities) -> Self {
		Self { inner }
	}

	/// Returns the identifier of the currently focused view.
	pub fn focused_view(&self) -> crate::hooks::ViewId {
		self.inner.focused_view()
	}

	/// Returns the current cursor position as a character index.
	pub fn cursor(&self) -> CharIdx {
		self.inner.cursor()
	}

	/// Returns the cursor position as (line, column), if available.
	pub fn cursor_line_col(&self) -> Option<(usize, usize)> {
		self.inner.cursor_line_col()
	}

	/// Sets the cursor position to the given character index.
	pub fn set_cursor(&mut self, pos: CharIdx) {
		self.inner.set_cursor(pos);
	}

	/// Returns a reference to the current selection.
	pub fn selection(&self) -> &Selection {
		self.inner.selection()
	}

	/// Sets the current selection.
	pub fn set_selection(&mut self, sel: Selection) {
		self.inner.set_selection(sel);
	}

	/// Sets the editor mode (Normal, Insert, etc.).
	pub fn set_mode(&mut self, mode: Mode) {
		self.inner.set_mode(mode);
	}

	/// Returns search access if the capability is available.
	pub fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		self.inner.search()
	}

	/// Returns search access or an error if not available.
	pub fn require_search(&mut self) -> Result<&mut dyn SearchAccess, CommandError> {
		self.inner.search().ok_or(CommandError::MissingCapability(Capability::Search))
	}

	/// Returns undo access if the capability is available.
	pub fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		self.inner.undo()
	}

	/// Returns undo access or an error if not available.
	pub fn require_undo(&mut self) -> Result<&mut dyn UndoAccess, CommandError> {
		self.inner.undo().ok_or(CommandError::MissingCapability(Capability::Undo))
	}

	/// Returns edit access if the capability is available.
	pub fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		self.inner.edit()
	}

	/// Returns edit access or an error if not available.
	pub fn require_edit(&mut self) -> Result<&mut dyn EditAccess, CommandError> {
		self.inner.edit().ok_or(CommandError::MissingCapability(Capability::Edit))
	}

	/// Returns motion access if the capability is available.
	pub fn motion(&mut self) -> Option<&mut dyn MotionAccess> {
		self.inner.motion()
	}

	/// Returns motion dispatch access if the capability is available.
	pub fn motion_dispatch(&mut self) -> Option<&mut dyn MotionDispatchAccess> {
		self.inner.motion_dispatch()
	}

	/// Returns split operations if the capability is available.
	pub fn split_ops(&mut self) -> Option<&mut dyn SplitOps> {
		self.inner.split_ops()
	}

	/// Returns focus operations if the capability is available.
	pub fn focus_ops(&mut self) -> Option<&mut dyn FocusOps> {
		self.inner.focus_ops()
	}

	/// Returns viewport access if the capability is available.
	pub fn viewport(&mut self) -> Option<&mut dyn ViewportAccess> {
		self.inner.viewport()
	}

	/// Returns jump list access if the capability is available.
	pub fn jump_ops(&mut self) -> Option<&mut dyn JumpAccess> {
		self.inner.jump_ops()
	}

	/// Returns macro operations if the capability is available.
	pub fn macro_ops(&mut self) -> Option<&mut dyn MacroAccess> {
		self.inner.macro_ops()
	}

	/// Returns deferred invocation access if the capability is available.
	pub fn deferred_invocations(&mut self) -> Option<&mut dyn DeferredInvocationAccess> {
		self.inner.deferred_invocations()
	}

	/// Returns overlay access if the capability is available.
	pub fn overlay(&mut self) -> Option<&mut dyn OverlayAccess> {
		self.inner.overlay()
	}

	/// Returns overlay access or an error if not available.
	pub fn require_overlay(&mut self) -> Result<&mut dyn OverlayAccess, CommandError> {
		self.inner.overlay().ok_or(CommandError::MissingCapability(Capability::Overlay))
	}

	/// Opens the command palette.
	pub fn open_palette(&mut self) {
		if let Some(p) = self.inner.palette() {
			p.open_palette();
		}
	}

	/// Closes the command palette without executing.
	pub fn close_palette(&mut self) {
		if let Some(p) = self.inner.palette() {
			p.close_palette();
		}
	}

	/// Executes the current palette input and closes it.
	pub fn execute_palette(&mut self) {
		if let Some(p) = self.inner.palette() {
			p.execute_palette();
		}
	}

	/// Opens the search prompt.
	pub fn open_search_prompt(&mut self, reverse: bool) {
		self.inner.open_search_prompt(reverse);
	}

	/// Returns whether the current buffer is read-only.
	pub fn is_readonly(&self) -> bool {
		self.inner.is_readonly()
	}

	/// Emits a type-safe notification.
	///
	/// # Example
	///
	/// ```ignore
	/// use crate::notifications::keys;
	/// ctx.emit(keys::BUFFER_READONLY);
	/// ctx.emit(keys::yanked_chars(42));
	/// ```
	pub fn emit(&mut self, notification: impl Into<crate::notifications::Notification>) {
		self.inner.emit(notification.into());
	}

	/// Checks if a specific capability is available.
	pub fn check_capability(&mut self, cap: Capability) -> bool {
		use Capability::*;
		match cap {
			Text | Cursor | Selection | Mode | Messaging => true,
			Edit => self.inner.edit().is_some(),
			Search => self.inner.search().is_some(),
			Undo => self.inner.undo().is_some(),
			FileOps => self.inner.file_ops().is_some(),
			Overlay => self.inner.overlay().is_some(),
		}
	}

	/// Checks if all specified capabilities are available.
	pub fn check_all_capabilities(&mut self, caps: &[Capability]) -> Result<(), CommandError> {
		for &cap in caps {
			if !self.check_capability(cap) {
				return Err(CommandError::MissingCapability(cap));
			}
		}
		Ok(())
	}

	/// Checks if all capabilities in the given set are available.
	pub fn check_capability_set(&mut self, caps: crate::CapabilitySet) -> Result<(), CommandError> {
		use crate::CapabilitySet;
		let all_caps = [
			(CapabilitySet::TEXT, Capability::Text),
			(CapabilitySet::CURSOR, Capability::Cursor),
			(CapabilitySet::SELECTION, Capability::Selection),
			(CapabilitySet::MODE, Capability::Mode),
			(CapabilitySet::MESSAGING, Capability::Messaging),
			(CapabilitySet::EDIT, Capability::Edit),
			(CapabilitySet::SEARCH, Capability::Search),
			(CapabilitySet::UNDO, Capability::Undo),
			(CapabilitySet::FILE_OPS, Capability::FileOps),
			(CapabilitySet::OVERLAY, Capability::Overlay),
		];
		for (flag, cap) in all_caps {
			if caps.contains(flag) && !self.check_capability(cap) {
				return Err(CommandError::MissingCapability(cap));
			}
		}
		Ok(())
	}

	/// Returns option access if the capability is available.
	pub fn option_ops(&self) -> Option<&dyn OptionAccess> {
		self.inner.option_ops()
	}
}

/// Core capabilities that all editors must provide for result handling.
///
/// Combines required capability traits ([`CursorAccess`], [`SelectionAccess`],
/// [`ModeAccess`], [`NotificationAccess`]) and provides optional accessors for
/// extended features. See module docs for why [`TextAccess`] is not required.
///
/// # Implementing
///
/// ```ignore
/// impl EditorCapabilities for MyEditor {
///     fn search(&mut self) -> Option<&mut dyn SearchAccess> {
///         Some(self)
///     }
/// }
/// ```
pub trait EditorCapabilities: CursorAccess + SelectionAccess + ModeAccess + NotificationAccess {
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

	/// Access to visual cursor motion (optional).
	fn motion(&mut self) -> Option<&mut dyn MotionAccess> {
		None
	}

	/// Access to motion dispatch with text access (optional).
	///
	/// This enables resolving motion IDs to handlers and applying them.
	fn motion_dispatch(&mut self) -> Option<&mut dyn MotionDispatchAccess> {
		None
	}

	/// Access to split management operations (optional).
	fn split_ops(&mut self) -> Option<&mut dyn SplitOps> {
		None
	}

	/// Access to focus and buffer navigation operations (optional).
	fn focus_ops(&mut self) -> Option<&mut dyn FocusOps> {
		None
	}

	/// Access to viewport queries (optional).
	fn viewport(&mut self) -> Option<&mut dyn ViewportAccess> {
		None
	}

	/// Access to file operations (optional).
	fn file_ops(&mut self) -> Option<&mut dyn FileOpsAccess> {
		None
	}

	/// Access to jump list operations (optional).
	fn jump_ops(&mut self) -> Option<&mut dyn JumpAccess> {
		None
	}

	/// Access to macro recording/playback operations (optional).
	fn macro_ops(&mut self) -> Option<&mut dyn MacroAccess> {
		None
	}

	/// Access to deferred invocation operations (optional).
	fn deferred_invocations(&mut self) -> Option<&mut dyn DeferredInvocationAccess> {
		None
	}

	/// Access to command palette operations (optional).
	fn palette(&mut self) -> Option<&mut dyn PaletteAccess> {
		None
	}

	/// Access to configuration option resolution (optional).
	fn option_ops(&self) -> Option<&dyn OptionAccess> {
		None
	}

	/// Access to UI overlays (optional).
	fn overlay(&mut self) -> Option<&mut dyn OverlayAccess> {
		None
	}

	/// Opens the search prompt.
	fn open_search_prompt(&mut self, _reverse: bool) {}

	/// Returns whether the current buffer is read-only.
	fn is_readonly(&self) -> bool {
		false
	}
}
