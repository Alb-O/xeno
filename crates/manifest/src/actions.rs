use ropey::RopeSlice;
use tome_base::Selection;
use tome_base::range::CharIdx;

use crate::{Capability, RegistrySource};

#[derive(Debug, Clone)]
pub enum ActionResult {
	Ok,
	ModeChange(ActionMode),
	CursorMove(CharIdx),
	Motion(Selection),
	InsertWithMotion(Selection),
	Edit(EditAction),
	Quit,
	ForceQuit,
	Error(String),
	Pending(PendingAction),
	SearchNext { add_selection: bool },
	SearchPrev { add_selection: bool },
	UseSelectionAsSearch,
	SplitLines,
	JumpForward,
	JumpBackward,
	SaveJump,
	RecordMacro,
	PlayMacro,
	SaveSelections,
	RestoreSelections,
	ForceRedraw,
	RepeatLastInsert,
	RepeatLastObject,
	DuplicateSelectionsDown,
	DuplicateSelectionsUp,
	MergeSelections,
	Align,
	CopyIndent,
	TabsToSpaces,
	SpacesToTabs,
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
	Command,
	SearchForward,
	SearchBackward,
	SelectRegex,
	SplitRegex,
	KeepMatching,
	KeepNotMatching,
	PipeReplace,
	PipeIgnore,
	InsertOutput,
	AppendOutput,
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
