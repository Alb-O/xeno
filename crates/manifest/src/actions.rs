use ropey::RopeSlice;
use tome_base::Selection;
use tome_base::range::CharIdx;

use crate::{Capability, RegistrySource};

/// Result of executing an action.
///
/// Actions return this enum to indicate what the editor should do next.
/// Variants are split into two categories based on whether they can be
/// applied when a terminal view is focused.
///
/// # Terminal-Safe Results
///
/// These operate at the workspace level and don't require text buffer context:
/// - [`Ok`], [`Quit`], [`ForceQuit`], [`Error`], [`ForceRedraw`]
/// - Split/buffer management: [`SplitHorizontal`], [`BufferNext`], [`CloseBuffer`], etc.
/// - Focus navigation: [`FocusLeft`], [`FocusRight`], [`FocusUp`], [`FocusDown`]
///
/// # Text Buffer Results
///
/// These require cursor, selection, or document access:
/// - Mode/cursor: [`ModeChange`], [`CursorMove`], [`Motion`]
/// - Editing: [`Edit`], [`SearchNext`], [`SplitLines`]
///
/// Use [`is_terminal_safe`] to check at runtime.
///
/// [`Ok`]: Self::Ok
/// [`Quit`]: Self::Quit
/// [`ForceQuit`]: Self::ForceQuit
/// [`Error`]: Self::Error
/// [`ForceRedraw`]: Self::ForceRedraw
/// [`SplitHorizontal`]: Self::SplitHorizontal
/// [`BufferNext`]: Self::BufferNext
/// [`CloseBuffer`]: Self::CloseBuffer
/// [`FocusLeft`]: Self::FocusLeft
/// [`FocusRight`]: Self::FocusRight
/// [`FocusUp`]: Self::FocusUp
/// [`FocusDown`]: Self::FocusDown
/// [`ModeChange`]: Self::ModeChange
/// [`CursorMove`]: Self::CursorMove
/// [`Motion`]: Self::Motion
/// [`Edit`]: Self::Edit
/// [`SearchNext`]: Self::SearchNext
/// [`SplitLines`]: Self::SplitLines
/// [`is_terminal_safe`]: Self::is_terminal_safe
#[derive(Debug, Clone)]
pub enum ActionResult {
	// Terminal-safe: workspace-level operations
	//
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

	// Text buffer required: cursor/selection/edit operations
	//
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
				| Self::Quit | Self::ForceQuit
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

/// Context passed to action handlers.
///
/// Provides read-only access to buffer state needed for computing action results.
/// Actions should not mutate state directly; instead, they return an [`ActionResult`]
/// that the editor applies.
pub struct ActionContext<'a> {
	/// Document text (read-only slice).
	pub text: RopeSlice<'a>,
	/// Current cursor position (char index).
	pub cursor: CharIdx,
	/// Current selection state.
	pub selection: &'a Selection,
	/// Repeat count (from numeric prefix, e.g., `3w` for 3 words).
	pub count: usize,
	/// Whether to extend the selection (shift held).
	pub extend: bool,
	/// Named register (e.g., `"a` for register 'a').
	pub register: Option<char>,
	/// Additional arguments from pending actions.
	pub args: ActionArgs,
}

/// Additional arguments for actions requiring extra input.
///
/// Used by pending actions that wait for user input (e.g., `f` waits for
/// a character to find, `r` waits for a replacement character).
#[derive(Debug, Clone, Default)]
pub struct ActionArgs {
	/// Single character argument (e.g., for `f`, `t`, `r` commands).
	pub char: Option<char>,
	/// String argument (e.g., for search patterns).
	pub string: Option<String>,
}

/// Definition of a registered action.
///
/// Actions are the fundamental unit of editor behavior. They're registered
/// at compile time via [`linkme`] distributed slices and looked up by keybindings.
///
/// # Registration
///
/// Use the `#[action]` proc macro in `tome-stdlib` to register actions:
///
/// ```ignore
/// #[action(id = "move_line_down", name = "Move Line Down")]
/// fn move_line_down(ctx: &ActionContext) -> ActionResult {
///     // ...
/// }
/// ```
pub struct ActionDef {
	/// Unique identifier (e.g., "tome-stdlib::move_line_down").
	pub id: &'static str,
	/// Human-readable name for UI display.
	pub name: &'static str,
	/// Alternative names for command lookup.
	pub aliases: &'static [&'static str],
	/// Description for help text.
	pub description: &'static str,
	/// The function that executes this action.
	pub handler: ActionHandler,
	/// Priority for conflict resolution (higher wins).
	pub priority: i16,
	/// Where this action was defined.
	pub source: RegistrySource,
	/// Capabilities required to execute this action.
	pub required_caps: &'static [Capability],
	/// Bitflags for additional behavior hints.
	pub flags: u32,
}

/// Function signature for action handlers.
///
/// Takes an immutable [`ActionContext`] and returns an [`ActionResult`]
/// describing what the editor should do.
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
