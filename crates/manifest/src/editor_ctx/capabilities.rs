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
//! - [`MessageAccess`] - Display notifications
//!
//! # Optional Traits
//!
//! These extend functionality when implemented:
//!
//! - [`EditAccess`] - Text modification (delete, yank, paste)
//! - [`SearchAccess`] - Pattern search and navigation
//! - [`UndoAccess`] - Undo/redo history
//! - [`BufferOpsAccess`] - Buffer/split management
//! - [`FileOpsAccess`] - Save/load operations
//!
//! # Not Yet Wired
//!
//! These traits are defined but not yet connected to [`EditorCapabilities`]:
//!
//! - [`JumpAccess`] - Jump list navigation
//! - [`MacroAccess`] - Macro recording/playback
//!
//! [`EditorCapabilities`]: super::EditorCapabilities

use evildoer_base::range::CharIdx;
use evildoer_base::selection::Selection;
use ropey::RopeSlice;

use crate::Mode;
use crate::actions::EditAction;

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

/// Message display and notifications (required).
///
/// Provides a way to display messages to the user in the status bar or
/// notification system.
pub trait MessageAccess {
	/// Displays a notification of the given type.
	///
	/// Common `type_id` values: "info", "warning", "error", "success".
	fn notify(&mut self, type_id: &str, msg: &str);

	/// Clears the current status message.
	fn clear_message(&mut self);
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

/// Jump list operations (not yet wired).
///
/// Provides navigation through the jump history. Jumps are saved automatically
/// when making large cursor movements. Add to [`Capability`] enum and implement
/// `jump()` accessor when ready.
///
/// [`Capability`]: crate::Capability
pub trait JumpAccess {
	/// Jumps forward in the jump list.
	fn jump_forward(&mut self) -> bool;
	/// Jumps backward in the jump list.
	fn jump_backward(&mut self) -> bool;
	/// Saves the current position to the jump list.
	fn save_jump(&mut self);
}

/// Macro recording/playback (not yet wired).
///
/// Enables recording sequences of actions and replaying them. Add to
/// [`Capability`] enum and implement `macros()` accessor when ready.
///
/// [`Capability`]: crate::Capability
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

/// Buffer and split management operations.
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
pub trait BufferOpsAccess {
	/// Split horizontally (new buffer below). Matches Vim `:split` / Helix `hsplit`.
	fn split_horizontal(&mut self);

	/// Split vertically (new buffer to right). Matches Vim `:vsplit` / Helix `vsplit`.
	fn split_vertical(&mut self);

	/// Toggle terminal split (open if closed, close if open).
	fn toggle_terminal(&mut self);
	/// Toggle the debug panel (open if closed, close if open).
	fn toggle_debug_panel(&mut self);
	/// Toggle a panel by name (open if closed, close if open).
	fn toggle_panel(&mut self, name: &str);
	/// Switch to the next buffer.
	fn buffer_next(&mut self);
	/// Switch to the previous buffer.
	fn buffer_prev(&mut self);
	/// Close the current split.
	fn close_split(&mut self);
	/// Close all other buffers.
	fn close_other_buffers(&mut self);
	/// Focus the split to the left.
	fn focus_left(&mut self);
	/// Focus the split to the right.
	fn focus_right(&mut self);
	/// Focus the split above.
	fn focus_up(&mut self);
	/// Focus the split below.
	fn focus_down(&mut self);
}

/// Convenience trait combining common capabilities for command handlers.
pub trait EditorOps: MessageAccess + FileOpsAccess + ThemeAccess {}
