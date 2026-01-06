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
//! - [`SearchAccess`] - Pattern search and navigation
//! - [`UndoAccess`] - Undo/redo history
//! - [`SplitOps`] - Split management
//! - [`FocusOps`] - Focus and buffer navigation
//! - [`ViewportAccess`] - Viewport position queries
//! - [`FileOpsAccess`] - Save/load operations
//! - [`JumpAccess`] - Jump list navigation
//! - [`MacroAccess`] - Macro recording/playback
//! - [`OptionAccess`] - Configuration option resolution
//!
//! [`EditorCapabilities`]: super::EditorCapabilities

use ropey::RopeSlice;
use xeno_base::range::CharIdx;
use xeno_base::selection::Selection;
use xeno_registry_notifications::Notification;
use xeno_registry_options::{FromOptionValue, OptionKey, OptionValue, TypedOptionKey};

use crate::{EditAction, Mode};

/// Cursor position access (required).
///
/// Provides read/write access to the primary cursor position in the document.
/// The cursor is a character index (not byte offset).
pub trait CursorAccess {
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
/// use xeno_registry_notifications::keys;
///
/// // Static message notification
/// ctx.emit(keys::buffer_readonly);
///
/// // Parameterized notification
/// ctx.emit(keys::yanked_chars::call(42));
/// ```
pub trait NotificationAccess {
	/// Emits a notification to the user.
	///
	/// Accepts any type that can be converted into a [`Notification`]:
	/// - [`NotificationKey`] - static message notifications (via `Into<Notification>`)
	/// - [`Notification`] - pre-built notifications from parameterized builders
	///
	/// [`NotificationKey`]: xeno_registry_notifications::NotificationKey
	/// [`Notification`]: xeno_registry_notifications::Notification
	fn emit(&mut self, notification: Notification);

	/// Clears all visible notifications.
	fn clear_notifications(&mut self);
}

/// Search operations (optional).
///
/// Enables pattern-based search and navigation. Supports multi-selection
/// search where each match can be added to the selection.
pub trait SearchAccess {
	/// Finds the next match. If `add_selection` is true, adds to selections.
	/// If `extend` is true, extends the current selection to include the match.
	fn search_next(&mut self, add_selection: bool, extend: bool) -> bool;
	/// Finds the previous match.
	fn search_prev(&mut self, add_selection: bool, extend: bool) -> bool;
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
	/// Saves the current state to the undo stack.
	fn save_state(&mut self);
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
/// [`ActionResult::Quit`] for that.
///
/// [`ActionResult::Quit`]: crate::ActionResult::Quit
pub trait EditAccess {
	/// Executes an edit action on the current selection.
	///
	/// If `extend` is true, the selection is extended rather than replaced
	/// during the operation.
	fn execute_edit(&mut self, action: &EditAction, extend: bool);
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
/// [`CommandError::Io`]: crate::CommandError::Io
/// [`CommandError::InvalidArgument`]: crate::CommandError::InvalidArgument
pub trait FileOpsAccess {
	/// Returns true if the buffer has unsaved changes.
	fn is_modified(&self) -> bool;
	/// Saves the buffer to its current file path.
	fn save(
		&mut self,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::CommandError>> + '_>>;
	/// Saves the buffer to a specific file path, updating the buffer's path.
	fn save_as(
		&mut self,
		path: std::path::PathBuf,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::CommandError>> + '_>>;
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
	/// [`CommandError::Failed`]: crate::CommandError::Failed
	fn set_theme(&mut self, name: &str) -> Result<(), crate::CommandError>;
}

/// Split management operations.
///
/// # Split Naming Convention
///
/// Split names refer to the orientation of the **split line**, matching Vim/Helix:
/// - `split_horizontal` = horizontal divider line → windows stacked top/bottom
/// - `split_vertical` = vertical divider line → windows side-by-side left/right
///
/// # Split Semantics
///
/// Splits create a new view sharing the same underlying document:
/// - Edits sync across all views of the same document
/// - Undo history is shared
/// - Each view has independent cursor, selection, and scroll position
pub trait SplitOps {
	/// Split horizontally (new buffer below). Matches Vim `:split` / Helix `hsplit`.
	fn split_horizontal(&mut self);

	/// Split vertically (new buffer to right). Matches Vim `:vsplit` / Helix `vsplit`.
	fn split_vertical(&mut self);

	/// Close the current split.
	fn close_split(&mut self);

	/// Close all other buffers.
	fn close_other_buffers(&mut self);
}

/// Focus and buffer navigation operations.
pub trait FocusOps {
	/// Switch to the next buffer.
	fn buffer_next(&mut self);
	/// Switch to the previous buffer.
	fn buffer_prev(&mut self);
	/// Focus the split to the left.
	fn focus_left(&mut self);
	/// Focus the split to the right.
	fn focus_right(&mut self);
	/// Focus the split above.
	fn focus_up(&mut self);
	/// Focus the split below.
	fn focus_down(&mut self);
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
/// returns [`ActionResult::Command`], the result handler uses this trait
/// to queue the command for execution on the next tick.
///
/// [`ActionResult::Command`]: crate::ActionResult::Command
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
/// use xeno_registry_options::keys;
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
	/// using `&dyn OptionAccess`, use [`option_raw()`] instead.
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry_options::keys;
	///
	/// let width: i64 = ctx.option(keys::TAB_WIDTH);
	/// let theme: String = ctx.option(keys::THEME);
	/// ```
	fn option<T: FromOptionValue>(&self, key: TypedOptionKey<T>) -> T
	where
		Self: Sized,
	{
		T::from_option(&self.option_raw(key.untyped()))
			.or_else(|| T::from_option(&(key.def().default)()))
			.expect("option type mismatch with registered default")
	}
}

/// Convenience trait combining common capabilities for command handlers.
pub trait EditorOps: NotificationAccess + FileOpsAccess + ThemeAccess {}
