pub use xeno_primitives::Mode;
use xeno_primitives::ScrollDirection;

/// Canonical dispatch payload emitted by keymap resolution.
#[derive(Debug, Clone)]
pub struct KeyDispatch {
	/// Invocation to execute through editor invocation dispatch.
	pub invocation: xeno_registry::Invocation,
}

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
	/// A canonical invocation to execute.
	Dispatch(KeyDispatch),
	/// Waiting for more keys to complete a sequence.
	///
	/// The UI should display a "which-key" style indicator showing
	/// that we're waiting for additional input.
	Pending {
		/// Number of keys accumulated so far.
		keys_so_far: usize,
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
