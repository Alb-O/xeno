//! Fine-grained capability traits for editor operations.

use ropey::RopeSlice;

use crate::Mode;
use crate::ext::actions::EditAction;
use crate::selection::Selection;

/// Cursor position access.
pub trait CursorAccess {
	fn cursor(&self) -> usize;
	fn set_cursor(&mut self, pos: usize);
}

/// Selection access.
pub trait SelectionAccess {
	fn selection(&self) -> &Selection;
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

/// Message display.
pub trait MessageAccess {
	fn show_message(&mut self, msg: &str);
	fn show_error(&mut self, msg: &str);
	fn clear_message(&mut self);
}

/// Scratch buffer operations.
pub trait ScratchAccess {
	fn open(&mut self, focus: bool);
	fn close(&mut self);
	fn toggle(&mut self);
	fn execute(&mut self) -> bool;
	fn is_open(&self) -> bool;
	fn is_focused(&self) -> bool;
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
	fn save_state(&mut self);
	fn undo(&mut self) -> bool;
	fn redo(&mut self) -> bool;
	fn can_undo(&self) -> bool;
	fn can_redo(&self) -> bool;
}

/// Selection manipulation operations.
pub trait SelectionOpsAccess {
	fn split_lines(&mut self) -> bool;
	fn merge_selections(&mut self);
	fn duplicate_down(&mut self);
	fn duplicate_up(&mut self);
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
	fn execute_edit(&mut self, action: &EditAction, extend: bool) -> bool;
}
