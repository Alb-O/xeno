//! Fine-grained capability traits for editor operations.

use ropey::RopeSlice;
use tome_base::range::CharIdx;
use tome_base::selection::Selection;

use crate::Mode;
use crate::actions::EditAction;

/// Cursor position access.
pub trait CursorAccess {
	fn cursor(&self) -> CharIdx;
	fn set_cursor(&mut self, pos: CharIdx);
}

/// Selection access.
pub trait SelectionAccess {
	fn selection(&self) -> &Selection;
	fn selection_mut(&mut self) -> &mut Selection;
	fn set_selection(&mut self, sel: Selection);
}

/// Document text access (read-only).
pub trait TextAccess {
	fn text(&self) -> RopeSlice<'_>;
}

/// Mode access.
pub trait ModeAccess {
	fn mode(&self) -> Mode;
	fn set_mode(&mut self, mode: Mode);
}

/// Message display and notifications.
pub trait MessageAccess {
	/// Generic notification entry point.
	fn notify(&mut self, type_id: &str, msg: &str);

	/// Clear the current message.
	fn clear_message(&mut self);
}

/// Search operations.
pub trait SearchAccess {
	fn search_next(&mut self, add_selection: bool, extend: bool) -> bool;
	fn search_prev(&mut self, add_selection: bool, extend: bool) -> bool;
	fn use_selection_as_pattern(&mut self) -> bool;
	fn pattern(&self) -> Option<&str>;
	fn set_pattern(&mut self, pattern: &str);
}

/// Undo/redo operations.
pub trait UndoAccess {
	/// Save current state to undo stack.
	fn save_state(&mut self);
	/// Undo the last change.
	fn undo(&mut self);
	/// Redo the last undone change.
	fn redo(&mut self);
	/// Check if undo is available.
	fn can_undo(&self) -> bool;
	/// Check if redo is available.
	fn can_redo(&self) -> bool;
}

/// Selection manipulation operations.
///
/// Note: `duplicate_down` and `duplicate_up` are handled via action result handlers
/// (see `unimplemented.rs`) and don't need trait methods since they operate on
/// the selection directly via `EditorContext`.
pub trait SelectionOpsAccess {
	/// Split the primary selection into per-line selections.
	fn split_lines(&mut self) -> bool;
	/// Merge overlapping and adjacent selections.
	fn merge_selections(&mut self);
}

/// Text transformation operations.
pub trait TransformAccess {
	fn align(&mut self);
	fn copy_indent(&mut self);
	fn tabs_to_spaces(&mut self);
	fn spaces_to_tabs(&mut self);
	fn trim_selections(&mut self);
}

/// Jump list operations.
pub trait JumpAccess {
	fn jump_forward(&mut self) -> bool;
	fn jump_backward(&mut self) -> bool;
	fn save_jump(&mut self);
}

/// Macro recording/playback.
pub trait MacroAccess {
	fn record(&mut self);
	fn stop_recording(&mut self);
	fn play(&mut self);
	fn is_recording(&self) -> bool;
}

/// Edit operations (delete, yank, paste, etc.).
pub trait EditAccess {
	/// Execute an edit action.
	///
	/// Edit actions modify the document content (delete, yank, paste, case changes, etc.)
	/// but never trigger application quit - use `ActionResult::Quit` for that.
	fn execute_edit(&mut self, action: &EditAction, extend: bool);
}

/// File operations (save, check modified state, etc.).
pub trait FileOpsAccess {
	/// Check if the buffer has unsaved changes.
	fn is_modified(&self) -> bool;
	/// Save the buffer to its current file path.
	fn save(
		&mut self,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::CommandError>> + '_>>;
	/// Save the buffer to a specific file path.
	fn save_as(
		&mut self,
		path: std::path::PathBuf,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::CommandError>> + '_>>;
}

/// Theme operations (get/set editor theme).
pub trait ThemeAccess {
	/// Set the editor theme by name.
	fn set_theme(&mut self, name: &str) -> Result<(), crate::CommandError>;
}

/// Buffer and split management operations.
pub trait BufferOpsAccess {
	/// Split the current view horizontally.
	fn split_horizontal(&mut self);
	/// Split the current view vertically.
	fn split_vertical(&mut self);
	/// Open a terminal in a horizontal split.
	fn split_terminal_horizontal(&mut self);
	/// Open a terminal in a vertical split.
	fn split_terminal_vertical(&mut self);
	/// Switch to the next buffer.
	fn buffer_next(&mut self);
	/// Switch to the previous buffer.
	fn buffer_prev(&mut self);
	/// Close the current buffer.
	fn close_buffer(&mut self);
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

/// Combined trait for command handlers - provides all common editor operations.
/// This is a convenience trait that combines the most commonly used capabilities.
pub trait EditorOps: TextAccess + MessageAccess + FileOpsAccess + ThemeAccess {}
