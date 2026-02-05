pub use xeno_primitives::Mode;
use xeno_primitives::key::ScrollDirection;
use xeno_registry::ActionId;

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
	/// An action to execute using typed ActionId (preferred).
	ActionById {
		/// The action identifier to execute.
		id: ActionId,
		/// Repeat count for the action.
		count: usize,
		/// Whether to extend selection instead of moving cursor.
		extend: bool,
		/// Register for yank/paste operations.
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
		/// The action identifier to execute.
		id: ActionId,
		/// Repeat count for the action.
		count: usize,
		/// Whether to extend selection instead of moving cursor.
		extend: bool,
		/// Register for yank/paste operations.
		register: Option<char>,
		/// Character argument for the action (e.g., find char target).
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
	MouseClick {
		/// Screen row (0-indexed).
		row: u16,
		/// Screen column (0-indexed).
		col: u16,
		/// Whether to extend selection instead of moving cursor.
		extend: bool,
	},
	/// Mouse drag to screen coordinates (extend selection).
	MouseDrag {
		/// Screen row (0-indexed).
		row: u16,
		/// Screen column (0-indexed).
		col: u16,
	},
	/// Mouse scroll.
	MouseScroll {
		/// Scroll direction (up, down, left, right).
		direction: ScrollDirection,
		/// Number of scroll units.
		count: usize,
	},
}
