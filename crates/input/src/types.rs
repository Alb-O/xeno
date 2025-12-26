use tome_base::key::ScrollDirection;
use tome_manifest::ActionId;
// Re-export Mode from tome-manifest
pub use tome_manifest::Mode;

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
	/// An action to execute using typed ActionId (preferred).
	ActionById {
		id: ActionId,
		count: usize,
		extend: bool,
		register: Option<char>,
	},
	/// An action with a character argument using typed ActionId (preferred).
	ActionByIdWithChar {
		id: ActionId,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: char,
	},
	/// An action to execute (string-based system, for backward compatibility).
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
