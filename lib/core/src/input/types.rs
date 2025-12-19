use crate::ext::PendingKind;
use crate::key::ScrollDirection;

/// Editor mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
	#[default]
	Normal,
	Insert,
	Goto,
	View,
	/// Command line input mode (for `:`, `/`, `?`, regex, pipe prompts).
	Command {
		prompt: char,
		input: String,
	},
	/// Waiting for character input to complete an action.
	PendingAction(PendingKind),
}

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
	/// An action to execute (string-based system).
	Action {
		name: &'static str,
		count: usize,
		extend: bool,
		register: Option<char>,
	},
	/// An action with a character argument (from pending completion).
	ActionWithChar {
		name: &'static str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: char,
	},
	/// Mode changed (to show in status).
	ModeChange(Mode),
	/// Key was consumed but no action needed.
	Consumed,
	/// Key was not handled.
	Unhandled,
	/// Insert a character (in insert mode).
	InsertChar(char),
	/// Execute a command-line command (from `:` prompt).
	ExecuteCommand(String),
	/// Execute a search (from `/` or `?` prompt).
	ExecuteSearch { pattern: String, reverse: bool },
	/// Select regex matches within selection (from `s` prompt).
	SelectRegex { pattern: String },
	/// Split selection on regex (from `S` prompt).
	SplitRegex { pattern: String },
	/// Keep selections matching regex (from `alt-k` prompt).
	KeepMatching { pattern: String },
	/// Keep selections not matching regex (from `alt-K` prompt).
	KeepNotMatching { pattern: String },
	/// Pipe selection through shell command, replace with output.
	PipeReplace { command: String },
	/// Pipe selection through shell command, ignore output.
	PipeIgnore { command: String },
	/// Insert shell command output before selection.
	InsertOutput { command: String },
	/// Append shell command output after selection.
	AppendOutput { command: String },
	/// Request to quit.
	Quit,
	/// Mouse click at screen coordinates.
	MouseClick { row: u16, col: u16, extend: bool },
	/// Mouse drag to screen coordinates (extend selection).
	MouseDrag { row: u16, col: u16 },
	/// Mouse scroll.
	MouseScroll {
		direction: ScrollDirection,
		count: usize,
	},
}
