//! Pending action state for multi-key sequences.
//!
//! When an action requires additional user input (e.g., `f` needs a character
//! to find), it returns [`ActionResult::Pending`] with a [`PendingAction`].

pub use xeno_base::PendingKind;

/// State for actions waiting on additional user input.
///
/// Created by [`ActionResult::Pending`] to signal that the editor should
/// capture more input before completing the action (e.g., `f` needs a char).
///
/// [`ActionResult::Pending`]: crate::ActionResult::Pending
#[derive(Debug, Clone)]
pub struct PendingAction {
	/// What type of input is expected.
	pub kind: PendingKind,
	/// Prompt to display while waiting.
	pub prompt: String,
}
