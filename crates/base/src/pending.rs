//!
//! Pending action state for additional user input.

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
