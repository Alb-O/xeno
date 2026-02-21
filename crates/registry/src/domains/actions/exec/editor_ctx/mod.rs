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
//! All capabilities are always available — there is no optional/`None` path.
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
pub use handlers::HandleOutcome;
use xeno_primitives::range::CharIdx;
use xeno_primitives::selection::Selection;

use crate::actions::{Capability, CommandError, Mode};

/// Context for applying action results to editor state.
///
/// Wraps an [`EditorCapabilities`] implementor and provides convenient methods
/// for common operations. Used by the result dispatch system to translate
/// [`ActionResult`] variants into editor mutations.
///
/// All capabilities are always available — access them directly:
///
/// ```ignore
/// ctx.search().search(direction, false, false);
/// ctx.edit().execute_edit_op(&op);
/// ```
///
/// [`ActionResult`]: crate::actions::ActionResult
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

	/// Returns search access.
	pub fn search(&mut self) -> &mut dyn SearchAccess {
		self.inner.search()
	}

	/// Returns undo access.
	pub fn undo(&mut self) -> &mut dyn UndoAccess {
		self.inner.undo()
	}

	/// Returns edit access.
	pub fn edit(&mut self) -> &mut dyn EditAccess {
		self.inner.edit()
	}

	/// Returns motion access.
	pub fn motion(&mut self) -> &mut dyn MotionAccess {
		self.inner.motion()
	}

	/// Returns motion dispatch access.
	pub fn motion_dispatch(&mut self) -> &mut dyn MotionDispatchAccess {
		self.inner.motion_dispatch()
	}

	/// Returns split operations.
	pub fn split_ops(&mut self) -> &mut dyn SplitOps {
		self.inner.split_ops()
	}

	/// Returns focus operations.
	pub fn focus_ops(&mut self) -> &mut dyn FocusOps {
		self.inner.focus_ops()
	}

	/// Returns viewport access.
	pub fn viewport(&mut self) -> &mut dyn ViewportAccess {
		self.inner.viewport()
	}

	/// Returns jump list access.
	pub fn jump_ops(&mut self) -> &mut dyn JumpAccess {
		self.inner.jump_ops()
	}

	/// Returns macro operations.
	pub fn macro_ops(&mut self) -> &mut dyn MacroAccess {
		self.inner.macro_ops()
	}

	/// Returns deferred invocation access.
	pub fn deferred_invocations(&mut self) -> &mut dyn DeferredInvocationAccess {
		self.inner.deferred_invocations()
	}

	/// Returns overlay access.
	pub fn overlay(&mut self) -> &mut dyn OverlayAccess {
		self.inner.overlay()
	}

	/// Opens the command palette.
	pub fn open_palette(&mut self) {
		self.inner.palette().open_palette();
	}

	/// Closes the command palette without executing.
	pub fn close_palette(&mut self) {
		self.inner.palette().close_palette();
	}

	/// Executes the current palette input and closes it.
	pub fn execute_palette(&mut self) {
		self.inner.palette().execute_palette();
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
	pub fn emit(&mut self, notification: impl Into<crate::notifications::Notification>) {
		self.inner.emit(notification.into());
	}

	/// All capabilities are always available. Returns `true` unconditionally.
	pub fn check_capability(&mut self, _cap: Capability) -> bool {
		true
	}

	/// All capabilities are always available. Returns `Ok(())` unconditionally.
	pub fn check_all_capabilities(&mut self, _caps: &[Capability]) -> Result<(), CommandError> {
		Ok(())
	}

	/// All capabilities are always available. Returns `Ok(())` unconditionally.
	pub fn check_capability_set(&mut self, _caps: crate::CapabilitySet) -> Result<(), CommandError> {
		Ok(())
	}

	/// Returns option access.
	pub fn option_ops(&self) -> &dyn OptionAccess {
		self.inner.option_ops()
	}
}

/// Full capability surface that all editors must provide for result handling.
///
/// Combines required capability traits ([`CursorAccess`], [`SelectionAccess`],
/// [`ModeAccess`], [`NotificationAccess`]) as supertraits and requires all
/// capability accessors. Every capability is always available — there is no
/// optional/`None` path.
///
/// # Implementing
///
/// ```ignore
/// impl EditorCapabilities for MyEditor {
///     fn search(&mut self) -> &mut dyn SearchAccess { self }
///     fn edit(&mut self) -> &mut dyn EditAccess { self }
///     // ... all accessors must be provided
/// }
/// ```
pub trait EditorCapabilities: CursorAccess + SelectionAccess + ModeAccess + NotificationAccess {
	/// Access to search operations.
	fn search(&mut self) -> &mut dyn SearchAccess;

	/// Access to undo/redo operations.
	fn undo(&mut self) -> &mut dyn UndoAccess;

	/// Access to edit operations.
	fn edit(&mut self) -> &mut dyn EditAccess;

	/// Access to visual cursor motion.
	fn motion(&mut self) -> &mut dyn MotionAccess;

	/// Access to motion dispatch with text access.
	fn motion_dispatch(&mut self) -> &mut dyn MotionDispatchAccess;

	/// Access to split management operations.
	fn split_ops(&mut self) -> &mut dyn SplitOps;

	/// Access to focus and buffer navigation operations.
	fn focus_ops(&mut self) -> &mut dyn FocusOps;

	/// Access to viewport queries.
	fn viewport(&mut self) -> &mut dyn ViewportAccess;

	/// Access to file operations.
	fn file_ops(&mut self) -> &mut dyn FileOpsAccess;

	/// Access to jump list operations.
	fn jump_ops(&mut self) -> &mut dyn JumpAccess;

	/// Access to macro recording/playback operations.
	fn macro_ops(&mut self) -> &mut dyn MacroAccess;

	/// Access to deferred invocation operations.
	fn deferred_invocations(&mut self) -> &mut dyn DeferredInvocationAccess;

	/// Access to command palette operations.
	fn palette(&mut self) -> &mut dyn PaletteAccess;

	/// Access to configuration option resolution.
	fn option_ops(&self) -> &dyn OptionAccess;

	/// Access to UI overlays.
	fn overlay(&mut self) -> &mut dyn OverlayAccess;

	/// Opens the search prompt.
	fn open_search_prompt(&mut self, _reverse: bool) {}

	/// Returns whether the current buffer is read-only.
	fn is_readonly(&self) -> bool {
		false
	}
}
