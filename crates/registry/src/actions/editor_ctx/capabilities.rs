//! Fine-grained capability traits for editor operations.
//!
//! Each trait represents a specific category of editor functionality. This allows
//! actions and commands to declare exactly what capabilities they need, and enables
//! graceful degradation when features aren't available.
//!
//! # Required Traits
//!
//! These must be implemented by all [`EditorCapabilities`] implementors:
//!
//! - [`CursorAccess`] - Get/set cursor position
//! - [`SelectionAccess`] - Get/set selections
//! - [`TextAccess`] - Read document content
//! - [`ModeAccess`] - Get/set editor mode
//! - [`NotificationAccess`] - Display notifications (type-safe)
//!
//! # Optional Traits
//!
//! These extend functionality when implemented:
//!
//! - [`EditAccess`] - Text modification (delete, yank, paste)
//! - [`MotionAccess`] - Visual/wrapped-line cursor movement
//! - [`SearchAccess`] - Pattern search and navigation
//! - [`UndoAccess`] - Undo/redo history
//! - [`SplitOps`] - Split management
//! - [`FocusOps`] - Focus and buffer navigation
//! - [`ViewportAccess`] - Viewport position queries
//! - [`FileOpsAccess`] - Save/load operations
//! - [`JumpAccess`] - Jump list navigation
//! - [`MacroAccess`] - Macro recording/playback
//! - [`OptionAccess`] - Configuration option resolution
//! - [`OverlayAccess`] - UI overlays and modal interactions
//!
//! [`EditorCapabilities`]: super::EditorCapabilities

use ropey::RopeSlice;
use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};
use xeno_primitives::range::{CharIdx, Direction};
use xeno_primitives::selection::Selection;

use crate::actions::effects::MotionRequest;
use crate::actions::{CommandError, Mode};
use crate::core::{FromOptionValue, OptionValue};
use crate::notifications::Notification;
use crate::options::{OptionKey, TypedOptionKey};

/// Cursor position access (required).
///
/// Provides read/write access to the primary cursor position in the document.
/// The cursor is a character index (not byte offset).
pub trait CursorAccess {
	/// Returns the identifier of the currently focused view.
	fn focused_view(&self) -> crate::hooks::ViewId;
	/// Returns the current cursor position as a character index.
	fn cursor(&self) -> CharIdx;
	/// Returns the cursor line and column, if available.
	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		None
	}
	/// Sets the cursor position.
	fn set_cursor(&mut self, pos: CharIdx);
}

/// Selection access (required).
///
/// Provides access to the editor's selection state. Multiple selections are
/// supported and the cursor is always part of a selection.
pub trait SelectionAccess {
	/// Returns a reference to the current selection.
	fn selection(&self) -> &Selection;
	/// Returns a mutable reference to the current selection.
	fn selection_mut(&mut self) -> &mut Selection;
	/// Replaces the current selection.
	fn set_selection(&mut self, sel: Selection);
}

/// Document text access (required, read-only).
///
/// Provides read-only access to the document content via [`ropey`]'s rope slice.
/// This is used by actions to compute motions and text objects.
pub trait TextAccess {
	/// Returns a read-only slice of the document text.
	fn text(&self) -> RopeSlice<'_>;
}

/// Mode access (required).
///
/// Controls the editor mode (Normal, Insert, Visual, etc.). The mode determines
/// how key input is interpreted.
pub trait ModeAccess {
	/// Returns the current editor mode.
	fn mode(&self) -> Mode;
	/// Changes the editor mode.
	fn set_mode(&mut self, mode: Mode);
}

/// Type-safe notification display (required).
///
/// Provides a way to display notifications to the user using typed keys.
///
/// # Example
///
/// ```ignore
/// use crate::notifications::keys;
///
/// // Static message notification
/// ctx.emit(keys::BUFFER_READONLY);
///
/// // Parameterized notification
/// ctx.emit(keys::yanked_chars(42));
/// ```
pub trait NotificationAccess {
	/// Emits a notification to the user.
	///
	/// Accepts any type that can be converted into a [`Notification`]:
	/// - [`NotificationKey`] - static message notifications (via `Into<Notification>`)
	/// - [`Notification`] - pre-built notifications from parameterized builders
	///
	/// [`NotificationKey`]: crate::notifications::NotificationKey
	/// [`Notification`]: crate::notifications::Notification
	fn emit(&mut self, notification: Notification);

	/// Clears all visible notifications.
	fn clear_notifications(&mut self);
}

/// Search operations (optional).
///
/// Enables pattern-based search and navigation. Supports multi-selection
/// search where each match can be added to the selection.
pub trait SearchAccess {
	/// Searches in the given direction.
	///
	/// - `direction`: `Next` for forward, `Prev` for backward
	/// - `add_selection`: if true, adds match to selections instead of replacing
	/// - `extend`: if true, extends the current selection to include the match
	fn search(&mut self, direction: SeqDirection, add_selection: bool, extend: bool) -> bool;
	/// Repeats the last search.
	///
	/// - `flip`: if true, searches in the opposite direction of the last search
	/// - `add_selection`: if true, adds match to selections instead of replacing
	/// - `extend`: if true, extends the current selection to include the match
	fn search_repeat(&mut self, flip: bool, add_selection: bool, extend: bool) -> bool;
	/// Uses the current selection text as the search pattern.
	fn use_selection_as_pattern(&mut self) -> bool;
	/// Returns the current search pattern, if any.
	fn pattern(&self) -> Option<&str>;
	/// Sets the search pattern.
	fn set_pattern(&mut self, pattern: &str);
}

/// Undo/redo operations (optional).
///
/// Provides access to the buffer's history stack for undoing and redoing changes.
pub trait UndoAccess {
	/// Undoes the last change.
	fn undo(&mut self);
	/// Redoes the last undone change.
	fn redo(&mut self);
	/// Returns true if undo is available.
	fn can_undo(&self) -> bool;
	/// Returns true if redo is available.
	fn can_redo(&self) -> bool;
}

/// Jump list operations.
///
/// Provides navigation through the jump history. Jumps are saved automatically
/// when making large cursor movements (e.g., searches, goto line).
pub trait JumpAccess {
	/// Jumps forward in the jump list.
	fn jump_forward(&mut self) -> bool;
	/// Jumps backward in the jump list.
	fn jump_backward(&mut self) -> bool;
	/// Saves the current position to the jump list.
	fn save_jump(&mut self);
}

/// Macro recording/playback.
///
/// Enables recording sequences of key events and replaying them.
pub trait MacroAccess {
	/// Starts recording a macro.
	fn record(&mut self);
	/// Stops recording the current macro.
	fn stop_recording(&mut self);
	/// Plays the recorded macro.
	fn play(&mut self);
	/// Returns true if currently recording a macro.
	fn is_recording(&self) -> bool;
}

/// Edit operations (optional).
///
/// Provides text modification capabilities including delete, yank, paste,
/// and case changes. Edit actions never trigger application quit; use
/// [`AppEffect::Quit`] for that.
///
/// [`AppEffect::Quit`]: crate::actions::effects::AppEffect::Quit
pub trait EditAccess {
	/// Executes a data-oriented edit operation.
	///
	/// This is the preferred method for text edits. EditOp records are
	/// composable and processed by a single executor function.
	fn execute_edit_op(&mut self, op: &crate::actions::edit_op::EditOp);

	/// Pastes from the yank register.
	///
	/// - `before`: If true, pastes before cursor; otherwise after
	fn paste(&mut self, before: bool);
}

/// Visual cursor motion (optional).
///
/// Handles cursor movement that accounts for visual line wrapping.
/// Unlike logical line movement, visual motion follows what the user
/// sees on screen when lines are wrapped.
pub trait MotionAccess {
	/// Moves the cursor visually (wrapped lines).
	///
	/// - `direction`: Forward for down, Backward for up
	/// - `count`: Number of visual lines to move
	/// - `extend`: If true, extends selection rather than moving
	fn move_visual_vertical(&mut self, direction: Direction, count: usize, extend: bool);
}

/// Motion dispatch via ID resolution.
///
/// Resolves [`MotionId`] to handlers and applies them. Separate from
/// [`MotionAccess`] because it requires document text access.
///
/// [`MotionId`]: xeno_primitives::MotionId
pub trait MotionDispatchAccess {
	/// Applies a motion request, returning the resulting selection.
	fn apply_motion(&mut self, req: &MotionRequest) -> Selection;
}

/// File operations (optional).
///
/// Provides save/load capabilities for buffers. Operations are async to support
/// non-blocking I/O.
///
/// # Errors
///
/// Save operations return [`CommandError::Io`] on filesystem errors,
/// [`CommandError::InvalidArgument`] for path issues.
///
/// [`CommandError::Io`]: crate::actions::CommandError::Io
/// [`CommandError::InvalidArgument`]: crate::actions::CommandError::InvalidArgument
pub trait FileOpsAccess {
	/// Returns true if the buffer has unsaved changes.
	fn is_modified(&self) -> bool;
	/// Saves the buffer to its current file path.
	fn save(
		&mut self,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), crate::actions::CommandError>> + '_>,
	>;
	/// Saves the buffer to a specific file path, updating the buffer's path.
	fn save_as(
		&mut self,
		path: std::path::PathBuf,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), crate::actions::CommandError>> + '_>,
	>;
}

/// Theme operations (optional).
///
/// Controls the editor's visual theme.
pub trait ThemeAccess {
	/// Sets the editor theme by name.
	///
	/// # Errors
	///
	/// Returns [`CommandError::Failed`] if the theme is not found.
	///
	/// [`CommandError::Failed`]: crate::actions::CommandError::Failed
	fn set_theme(&mut self, name: &str) -> Result<(), crate::actions::CommandError>;
}

/// Errors that can occur during split operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitError {
	/// The target view was not found in any layout layer.
	ViewNotFound,
	/// The view area is too small to split.
	AreaTooSmall,
}

/// Split management operations.
///
/// # Split Naming Convention
///
/// Split names refer to the orientation of the **split line**, matching Vim/Helix:
/// - `Axis::Horizontal` = horizontal divider line → windows stacked top/bottom
/// - `Axis::Vertical` = vertical divider line → windows side-by-side left/right
///
/// # Split Semantics
///
/// Splits create a new view sharing the same underlying document:
/// - Edits sync across all views of the same document
/// - Undo history is shared
/// - Each view has independent cursor, selection, and scroll position
///
/// # Atomicity
///
/// Split operations are atomic: if they return an error, no state changes
/// have occurred (no buffer allocated, no layout modified, no focus changed).
pub trait SplitOps {
	/// Split along the given axis. See trait docs for axis semantics.
	///
	/// Returns `Ok(())` on success, or an error if the split cannot be created.
	fn split(&mut self, axis: Axis) -> Result<(), SplitError>;

	/// Close the current split.
	fn close_split(&mut self);

	/// Close all other buffers.
	fn close_other_buffers(&mut self);
}

/// Focus and buffer navigation operations.
pub trait FocusOps {
	/// Switch buffer in the given direction (next/previous).
	fn buffer_switch(&mut self, direction: SeqDirection);

	/// Focus the split in the given spatial direction.
	fn focus(&mut self, direction: SpatialDirection);
}

/// Viewport query operations (optional).
pub trait ViewportAccess {
	/// Returns the last rendered viewport height in rows.
	fn viewport_height(&self) -> usize;
	/// Converts a viewport row to a document character position.
	fn viewport_row_to_doc_position(&self, row: usize) -> Option<CharIdx>;
}

/// Command queue operations (optional).
///
/// Allows actions to schedule commands for async execution. When an action
/// returns [`AppEffect::QueueCommand`], the result handler uses this trait
/// to queue the command for execution on the next tick.
///
/// [`AppEffect::QueueCommand`]: crate::actions::effects::AppEffect::QueueCommand
pub trait CommandQueueAccess {
	/// Queues a command for async execution.
	///
	/// The command will be executed by the main loop on the next tick,
	/// with full async context and editor access.
	fn queue_command(&mut self, name: &'static str, args: Vec<String>);
}

/// Command palette operations.
///
/// Opens, closes, and executes the command palette floating input.
pub trait PaletteAccess {
	/// Opens the command palette.
	fn open_palette(&mut self);
	/// Closes the command palette without executing.
	fn close_palette(&mut self);
	/// Executes the current palette input and closes it.
	fn execute_palette(&mut self);
	/// Returns true if the palette is currently open.
	fn palette_is_open(&self) -> bool;
}

/// Access to configuration options (optional).
///
/// Provides context-aware resolution of configuration options through the
/// layered hierarchy: buffer-local -> language -> global -> default.
///
/// # Example
///
/// ```ignore
/// use crate::options::keys;
///
/// // Get tab width for current buffer context (typed)
/// let width: i64 = ctx.option(keys::TAB_WIDTH);
///
/// // Get theme (global option)
/// let theme: String = ctx.option(keys::THEME);
/// ```
pub trait OptionAccess {
	/// Resolves an option for the current context (buffer-aware).
	///
	/// Resolution order:
	/// 1. Buffer-local override (from `:setlocal`)
	/// 2. Language-specific config (from `language "rust" { }` block)
	/// 3. Global config (from `options { }` block)
	/// 4. Compile-time default (from `#[derive_option]` macro)
	fn option_raw(&self, key: OptionKey) -> OptionValue;

	/// Resolves a typed option for the current context.
	///
	/// This is the preferred method for option access, providing compile-time
	/// type safety through [`TypedOptionKey<T>`].
	///
	/// Note: This method requires `Self: Sized` for dyn-compatibility. When
	/// using `&dyn OptionAccess`, use [`Self::option_raw`] instead.
	///
	/// # Example
	///
	/// ```ignore
	/// use crate::options::keys;
	///
	/// let width: i64 = ctx.option(keys::TAB_WIDTH);
	/// let theme: String = ctx.option(keys::THEME);
	/// ```
	fn option<T: FromOptionValue>(&self, key: TypedOptionKey<T>) -> T
	where
		Self: Sized,
	{
		T::from_option(&self.option_raw(key.untyped()))
			.or_else(|| T::from_option(&key.def().default.to_value()))
			.expect("option type mismatch with registered default")
	}
}

/// Stable, editor-agnostic overlay requests.
/// Keep this SMALL; add variants only when you have a real caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayRequest {
	/// Open a named modal overlay (editor resolves name -> controller).
	OpenModal {
		kind: &'static str,
		args: Vec<String>,
	},

	/// Close the active modal overlay (if any).
	CloseModal { reason: OverlayCloseReason },

	/// Show a passive info popup (non-modal). Concrete rendering is editor-owned.
	ShowInfoPopup { title: Option<String>, body: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayCloseReason {
	Cancel,
	Commit,
	Blur,
	Forced,
}

/// Optional capability trait.
/// Implemented by the real editor; test harnesses can omit it.
pub trait OverlayAccess {
	/// Apply an overlay request. Editors MAY treat some requests as no-ops
	/// if overlays are not enabled in the current mode.
	fn overlay_request(&mut self, req: OverlayRequest) -> Result<(), CommandError>;

	/// Optional query helper (useful for guards in result handlers).
	fn overlay_modal_is_open(&self) -> bool {
		false
	}
}

/// Convenience trait combining common capabilities for command handlers.
pub trait EditorOps: NotificationAccess + FileOpsAccess + ThemeAccess {}
