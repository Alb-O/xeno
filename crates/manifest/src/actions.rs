use ropey::RopeSlice;
use tome_base::Selection;
use tome_base::range::{CharIdx, Range};
use tome_macro::DispatchResult;

use crate::{Capability, RegistrySource, find_motion};

/// Result of executing an action.
///
/// Actions return this enum to indicate what the editor should do next.
/// Variants marked `#[terminal_safe]` can be applied when a terminal view
/// is focused (workspace-level operations). Other variants require text
/// buffer context.
///
/// The `#[derive(DispatchResult)]` macro generates:
/// - Handler slices (`RESULT_*_HANDLERS`) for each variant
/// - [`dispatch_result`] function for routing results to handlers
/// - [`is_terminal_safe`] method from `#[terminal_safe]` attributes
///
/// [`dispatch_result`]: crate::dispatch_result
/// [`is_terminal_safe`]: Self::is_terminal_safe
#[derive(Debug, Clone, DispatchResult)]
pub enum ActionResult {
	/// No-op success.
	#[terminal_safe]
	Ok,
	/// Quit the editor.
	#[terminal_safe]
	#[handler(Quit)]
	Quit,
	/// Force quit without save prompts.
	#[terminal_safe]
	#[handler(Quit)]
	ForceQuit,
	/// Error message to display.
	#[terminal_safe]
	Error(String),
	/// Force a redraw.
	#[terminal_safe]
	ForceRedraw,
	/// Split horizontally with current buffer.
	#[terminal_safe]
	SplitHorizontal,
	/// Split vertically with current buffer.
	#[terminal_safe]
	SplitVertical,
	/// Open terminal in horizontal split.
	#[terminal_safe]
	SplitTerminalHorizontal,
	/// Open terminal in vertical split.
	#[terminal_safe]
	SplitTerminalVertical,
	/// Switch to next buffer.
	#[terminal_safe]
	BufferNext,
	/// Switch to previous buffer.
	#[terminal_safe]
	BufferPrev,
	/// Close current buffer/view.
	#[terminal_safe]
	CloseBuffer,
	/// Close all other buffers.
	#[terminal_safe]
	CloseOtherBuffers,
	/// Focus split to the left.
	#[terminal_safe]
	FocusLeft,
	/// Focus split to the right.
	#[terminal_safe]
	FocusRight,
	/// Focus split above.
	#[terminal_safe]
	FocusUp,
	/// Focus split below.
	#[terminal_safe]
	FocusDown,

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
	#[handler(UseSelectionSearch)]
	UseSelectionAsSearch,
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

/// Applies a named motion as a cursor movement.
///
/// Moves all cursor positions according to the motion. When not extending,
/// collapses selections to points at the new head positions.
pub fn cursor_motion(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let Some(motion) = find_motion(motion_name) else {
		return ActionResult::Error(format!("Unknown motion: {}", motion_name));
	};

	let new_ranges: Vec<Range> = ctx
		.selection
		.ranges()
		.iter()
		.map(|range| {
			let seed = if ctx.extend {
				*range
			} else {
				Range::point(range.head)
			};
			let moved = (motion.handler)(ctx.text, seed, ctx.count, ctx.extend);
			if ctx.extend {
				moved
			} else {
				Range::point(moved.head)
			}
		})
		.collect();

	ActionResult::Motion(Selection::from_vec(
		new_ranges,
		ctx.selection.primary_index(),
	))
}

/// Applies a named motion as a selection-creating action.
///
/// Creates or extends selections from current positions to new positions
/// determined by the motion. Used for word motions and similar commands.
pub fn selection_motion(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let Some(motion) = find_motion(motion_name) else {
		return ActionResult::Error(format!("Unknown motion: {}", motion_name));
	};

	if ctx.extend {
		let primary_index = ctx.selection.primary_index();
		let new_ranges: Vec<Range> = ctx
			.selection
			.ranges()
			.iter()
			.enumerate()
			.map(|(i, range)| {
				let seed = if i == primary_index {
					Range::new(range.anchor, ctx.cursor)
				} else {
					*range
				};
				(motion.handler)(ctx.text, seed, ctx.count, true)
			})
			.collect();
		ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
	} else {
		let current_range = Range::point(ctx.cursor);
		let new_range = (motion.handler)(ctx.text, current_range, ctx.count, false);
		ActionResult::Motion(Selection::single(new_range.anchor, new_range.head))
	}
}
