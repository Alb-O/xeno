//! Action system for extensible commands and motions.
//!
//! Actions are the unified abstraction for all editor operations that can be
//! triggered by keybindings. This replaces the hardcoded `Command` enum with
//! a dynamic, extensible registry.

mod delete;
mod editing;
mod find;
mod insert;
mod misc;
mod modes;
mod motions;
mod pipe;
mod regex_select;
mod scroll;
mod search;
mod selection_ops;
mod text_objects;

use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::selection::Selection;

/// Registry of all actions, populated at link time.
#[distributed_slice]
pub static ACTIONS: [ActionDef];

/// The result of executing an action.
#[derive(Debug, Clone)]
pub enum ActionResult {
	/// Action completed successfully, no state change.
	Ok,
	/// Action requests a mode change.
	ModeChange(ActionMode),
	/// Action is a cursor movement (moves cursor only, preserves selections).
	CursorMove(crate::range::CharIdx),
	/// Action is a motion that produces a new selection.
	Motion(Selection),
	/// Apply motion then enter insert mode.
	InsertWithMotion(Selection),
	/// Action modifies the document (delete, insert, etc.).
	Edit(EditAction),
	/// Action requests quitting the editor.
	Quit,
	/// Action requests quitting without saving.
	ForceQuit,
	/// Action failed with an error message.
	Error(String),
	/// Action needs more input (e.g., awaiting a character for 'f' find).
	Pending(PendingAction),
	/// Go to next search match.
	SearchNext { add_selection: bool },
	/// Go to previous search match.
	SearchPrev { add_selection: bool },
	/// Use selection as search pattern and go to next match.
	UseSelectionAsSearch,
	/// Split selection into lines.
	SplitLines,
	/// Jump forward in jump list.
	JumpForward,
	/// Jump backward in jump list.
	JumpBackward,
	/// Save current position to jump list.
	SaveJump,
	/// Record or stop recording macro.
	RecordMacro,
	/// Play macro.
	PlayMacro,
	/// Save current selections to mark.
	SaveSelections,
	/// Restore selections from mark.
	RestoreSelections,
	/// Force redraw of the screen.
	ForceRedraw,
	/// Repeat the last insert/change action.
	RepeatLastInsert,
	/// Repeat the last object/find operation.
	RepeatLastObject,
	/// Duplicate selections on next lines (C).
	DuplicateSelectionsDown,
	/// Duplicate selections on previous lines (alt-C).
	DuplicateSelectionsUp,
	/// Merge overlapping selections (alt-+).
	MergeSelections,
	/// Align cursors (&).
	Align,
	/// Copy indent from previous line (alt-&).
	CopyIndent,
	/// Convert tabs to spaces (@).
	TabsToSpaces,
	/// Convert spaces to tabs (alt-@).
	SpacesToTabs,
	/// Trim whitespace from selections (_).
	TrimSelections,
}

/// An edit operation to apply to the document.
#[derive(Debug, Clone)]
pub enum EditAction {
	/// Delete the current selection (optionally yank first).
	Delete { yank: bool },
	/// Delete selection and enter insert mode.
	Change { yank: bool },
	/// Yank the current selection to the register.
	Yank,
	/// Paste from register.
	Paste { before: bool },
	/// Paste all register contents.
	PasteAll { before: bool },
	/// Replace selection with character.
	ReplaceWithChar { ch: char },
	/// Undo the last change.
	Undo,
	/// Redo the last undone change.
	Redo,
	/// Indent the selection.
	Indent,
	/// Deindent the selection.
	Deindent,
	/// Convert selection to lowercase.
	ToLowerCase,
	/// Convert selection to uppercase.
	ToUpperCase,
	/// Swap case of selection.
	SwapCase,
	/// Join lines.
	JoinLines,
	/// Delete character before cursor (backspace).
	DeleteBack,
	/// Open a new line below and enter insert mode.
	OpenBelow,
	/// Open a new line above and enter insert mode.
	OpenAbove,
	/// Move cursor visually (respects soft wrap).
	MoveVisual {
		direction: VisualDirection,
		count: usize,
		extend: bool,
	},
	/// Scroll view and move cursor (PageUp/PageDown behavior).
	Scroll {
		direction: ScrollDir,
		amount: ScrollAmount,
		extend: bool,
	},
	/// Add empty line below (without entering insert mode).
	AddLineBelow,
	/// Add empty line above (without entering insert mode).
	AddLineAbove,
}

/// Direction for visual movement (respects soft wrap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualDirection {
	Up,
	Down,
}

/// Direction for scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDir {
	Up,
	Down,
}

/// Amount to scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAmount {
	Line(usize),
	HalfPage,
	FullPage,
}

/// Mode to switch to after an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMode {
	Normal,
	Insert,
	Goto,
	View,
	Command,
	SearchForward,
	SearchBackward,
	/// Select regex matches within selection (s)
	SelectRegex,
	/// Split selection on regex (S)
	SplitRegex,
	/// Keep selections matching regex (alt-k)
	KeepMatching,
	/// Keep selections not matching regex (alt-K)
	KeepNotMatching,
	/// Pipe through shell command, replace selection (|)
	PipeReplace,
	/// Pipe through shell command, ignore output (alt-|)
	PipeIgnore,
	/// Insert shell command output (!)
	InsertOutput,
	/// Append shell command output (alt-!)
	AppendOutput,
}

/// An action that needs additional input to complete.
#[derive(Debug, Clone)]
pub struct PendingAction {
	pub kind: PendingKind,
	pub prompt: String,
}

/// The kind of pending input needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
	/// Find character forward (f/t)
	FindChar { inclusive: bool },
	/// Find character backward (alt-f/alt-t)
	FindCharReverse { inclusive: bool },
	/// Replace with character (r)
	ReplaceChar,
	/// Select text object (alt-i/alt-a/[/])
	Object(ObjectSelectionKind),
}

/// How to select a text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSelectionKind {
	/// Select content inside delimiters (alt-i).
	Inner,
	/// Select content including delimiters (alt-a).
	Around,
	/// Select from cursor to object start ([).
	ToStart,
	/// Select from cursor to object end (]).
	ToEnd,
}

/// Context passed to action handlers.
pub struct ActionContext<'a> {
	/// Slice of the entire buffer.
	pub text: RopeSlice<'a>,
	/// Current primary cursor position.
	pub cursor: crate::range::CharIdx,
	/// Current selections.
	pub selection: &'a Selection,
	/// Repeat count (from numeric prefix).
	pub count: usize,
	/// Whether to extend existing selections (shift-movement).
	pub extend: bool,
	/// The register to use for the action.
	pub register: Option<char>,
	/// Additional arguments (e.g., character for find).
	pub args: ActionArgs,
}

/// Additional arguments for actions.
#[derive(Debug, Clone, Default)]
pub struct ActionArgs {
	/// Character argument (for f/t/r commands).
	pub char: Option<char>,
	/// String argument (for search, etc.).
	pub string: Option<String>,
}

/// Definition of an action that can be registered.
pub struct ActionDef {
	/// Unique identifier (usually same as name).
	pub id: &'static str,
	/// Unique name for the action (e.g., "move_left", "delete_selection").
	pub name: &'static str,
	/// Alternative names for the action.
	pub aliases: &'static [&'static str],
	/// Human-readable description.
	pub description: &'static str,
	/// The action handler function.
	pub handler: ActionHandler,
	/// Priority for resolving name collisions.
	pub priority: i16,
	/// Origin of the action.
	pub source: crate::ext::ExtensionSource,
	/// Capabilities required to run this action.
	pub required_caps: &'static [crate::ext::Capability],
	/// Metadata flags.
	pub flags: u32,
}

/// The type of action handler functions.
pub type ActionHandler = fn(&ActionContext) -> ActionResult;

/// Look up an action by name.
pub use crate::ext::index::find_action;

/// Execute an action by name with the given context.
pub fn execute_action(name: &str, ctx: &ActionContext) -> ActionResult {
	match find_action(name) {
		Some(action) => (action.handler)(ctx),
		None => ActionResult::Error(format!("Unknown action: {}", name)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_action_unknown() {
		assert!(find_action("nonexistent_action_xyz").is_none());
	}

	#[test]
	fn test_motion_actions_registered() {
		assert!(find_action("move_left").is_some());
		assert!(find_action("move_right").is_some());
		assert!(find_action("move_up").is_some());
		assert!(find_action("move_down").is_some());
		assert!(find_action("move_line_start").is_some());
		assert!(find_action("move_line_end").is_some());
		assert!(find_action("next_word_start").is_some());
		assert!(find_action("document_start").is_some());
		assert!(find_action("document_end").is_some());
	}
}
