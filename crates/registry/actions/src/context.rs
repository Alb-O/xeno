//! Action execution context.
//!
//! Provides read-only access to buffer state needed for computing action results.

use evildoer_base::Selection;
use evildoer_base::range::CharIdx;
use ropey::RopeSlice;

/// Context passed to action handlers.
///
/// Provides read-only access to buffer state needed for computing action results.
/// Actions should not mutate state directly; instead, they return an [`ActionResult`]
/// that the editor applies.
///
/// [`ActionResult`]: crate::ActionResult
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
