use evildoer_base::key::ScrollDirection;
use evildoer_manifest::ActionId;
// Re-export Mode from evildoer-manifest
pub use evildoer_manifest::Mode;

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
	/// Waiting for more keys to complete a sequence.
	///
	/// The UI should display a "which-key" style indicator showing
	/// that we're waiting for additional input.
	Pending {
		/// Number of keys accumulated so far.
		keys_so_far: usize,
	},
	/// An action with a character argument using typed ActionId.
	ActionByIdWithChar {
		id: ActionId,
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
