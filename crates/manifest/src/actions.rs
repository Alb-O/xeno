use ropey::RopeSlice;
use tome_base::Selection;
use tome_base::range::CharIdx;

use crate::{Capability, RegistrySource};

/// Result of executing an action.
///
/// Each variant is marked as either terminal-safe or not. Terminal-safe results
/// can be applied when a terminal view is focused; others require a text buffer.
#[derive(Debug, Clone)]
pub enum ActionResult {
	// === Terminal-safe: workspace-level operations ===
	/// No-op success.
	Ok,
	/// Quit the editor.
	Quit,
	/// Force quit without save prompts.
	ForceQuit,
	/// Error message to display.
	Error(String),
	/// Force a redraw.
	ForceRedraw,
	/// Split horizontally with current buffer.
	SplitHorizontal,
	/// Split vertically with current buffer.
	SplitVertical,
	/// Open terminal in horizontal split.
	SplitTerminalHorizontal,
	/// Open terminal in vertical split.
	SplitTerminalVertical,
	/// Switch to next buffer.
	BufferNext,
	/// Switch to previous buffer.
	BufferPrev,
	/// Close current buffer/view.
	CloseBuffer,
	/// Close all other buffers.
	CloseOtherBuffers,
	/// Focus split to the left.
	FocusLeft,
	/// Focus split to the right.
	FocusRight,
	/// Focus split above.
	FocusUp,
	/// Focus split below.
	FocusDown,

	// === Text buffer required: cursor/selection/edit operations ===
	/// Change editor mode.
	ModeChange(ActionMode),
	/// Move cursor to position.
	CursorMove(CharIdx),
	/// Apply a motion (updates selection).
	Motion(Selection),
	/// Enter insert mode with motion.
	InsertWithMotion(Selection),
	/// Execute an edit action.
	Edit(EditAction),
	/// Enter pending state for multi-key action.
	Pending(PendingAction),
	/// Search forward.
	SearchNext { add_selection: bool },
	/// Search backward.
	SearchPrev { add_selection: bool },
	/// Use current selection as search pattern.
	UseSelectionAsSearch,
	/// Split selection into lines.
	SplitLines,
	/// Jump forward in jump list.
	JumpForward,
	/// Jump backward in jump list.
	JumpBackward,
	/// Save current position to jump list.
	SaveJump,
	/// Start/stop macro recording.
	RecordMacro,
	/// Play recorded macro.
	PlayMacro,
	/// Save current selections.
	SaveSelections,
	/// Restore saved selections.
	RestoreSelections,
	/// Repeat last insert.
	RepeatLastInsert,
	/// Repeat last text object.
	RepeatLastObject,
	/// Duplicate selections downward.
	DuplicateSelectionsDown,
	/// Duplicate selections upward.
	DuplicateSelectionsUp,
	/// Merge overlapping selections.
	MergeSelections,
	/// Align selections.
	Align,
	/// Copy indentation.
	CopyIndent,
	/// Convert tabs to spaces.
	TabsToSpaces,
	/// Convert spaces to tabs.
	SpacesToTabs,
	/// Trim whitespace from selections.
	TrimSelections,
}

impl ActionResult {
	/// Returns true if this result can be applied when a terminal is focused.
	///
	/// Terminal-safe results are workspace-level operations that don't require
	/// text buffer context (cursor, selection, document content).
	pub fn is_terminal_safe(&self) -> bool {
		matches!(
			self,
			Self::Ok
				| Self::Quit
				| Self::ForceQuit
				| Self::Error(_)
				| Self::ForceRedraw
				| Self::SplitHorizontal
				| Self::SplitVertical
				| Self::SplitTerminalHorizontal
				| Self::SplitTerminalVertical
				| Self::BufferNext
				| Self::BufferPrev
				| Self::CloseBuffer
				| Self::CloseOtherBuffers
				| Self::FocusLeft
				| Self::FocusRight
				| Self::FocusUp
				| Self::FocusDown
		)
	}
}

#[derive(Debug, Clone)]
pub enum EditAction {
	Delete {
		yank: bool,
	},
	Change {
		yank: bool,
	},
	Yank,
	Paste {
		before: bool,
	},
	PasteAll {
		before: bool,
	},
	ReplaceWithChar {
		ch: char,
	},
	Undo,
	Redo,
	Indent,
	Deindent,
	ToLowerCase,
	ToUpperCase,
	SwapCase,
	JoinLines,
	DeleteBack,
	OpenBelow,
	OpenAbove,
	MoveVisual {
		direction: VisualDirection,
		count: usize,
		extend: bool,
	},
	Scroll {
		direction: ScrollDir,
		amount: ScrollAmount,
		extend: bool,
	},
	AddLineBelow,
	AddLineAbove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualDirection {
	Up,
	Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDir {
	Up,
	Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAmount {
	Line(usize),
	HalfPage,
	FullPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMode {
	Normal,
	Insert,
	Goto,
	View,
	Window,
}

#[derive(Debug, Clone)]
pub struct PendingAction {
	pub kind: PendingKind,
	pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
	FindChar { inclusive: bool },
	FindCharReverse { inclusive: bool },
	ReplaceChar,
	Object(ObjectSelectionKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSelectionKind {
	Inner,
	Around,
	ToStart,
	ToEnd,
}

pub struct ActionContext<'a> {
	pub text: RopeSlice<'a>,
	pub cursor: CharIdx,
	pub selection: &'a Selection,
	pub count: usize,
	pub extend: bool,
	pub register: Option<char>,
	pub args: ActionArgs,
}

#[derive(Debug, Clone, Default)]
pub struct ActionArgs {
	pub char: Option<char>,
	pub string: Option<String>,
}

pub struct ActionDef {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub description: &'static str,
	pub handler: ActionHandler,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

pub type ActionHandler = fn(&ActionContext) -> ActionResult;

impl crate::RegistryMetadata for ActionDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}
