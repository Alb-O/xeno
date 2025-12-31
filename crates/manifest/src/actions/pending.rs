//! Pending action state for multi-key sequences.
//!
//! When an action requires additional user input (e.g., `f` needs a character
//! to find), it returns [`ActionResult::Pending`] with a [`PendingAction`].

/// State for actions waiting on additional user input.
///
/// Created by [`ActionResult::Pending`] to signal that the editor should
/// capture more input before completing the action (e.g., `f` needs a char).
///
/// [`ActionResult::Pending`]: super::ActionResult::Pending
#[derive(Debug, Clone)]
pub struct PendingAction {
	/// What type of input is expected.
	pub kind: PendingKind,
	/// Prompt to display while waiting.
	pub prompt: String,
}

/// Type of pending action awaiting input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
	/// Find character forward (`f`/`t` commands).
	FindChar { inclusive: bool },
	/// Find character backward (`F`/`T` commands).
	FindCharReverse { inclusive: bool },
	/// Replace character under cursor (`r` command).
	ReplaceChar,
	/// Select text object (`i`/`a` after operator).
	Object(ObjectSelectionKind),
}

/// How to select a text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSelectionKind {
	/// Select inside delimiters (e.g., `iw` for inner word).
	Inner,
	/// Select including delimiters (e.g., `aw` for around word).
	Around,
	/// Select from cursor to object start.
	ToStart,
	/// Select from cursor to object end.
	ToEnd,
}
