//! Pending action state for multi-key sequences.
//!
//! When an action requires additional user input (e.g., `f` needs a character
//! to find), it returns [`ActionResult::Effects`] containing an [`AppEffect::Pending`]
//! with a [`PendingAction`].
//!
//! [`ActionResult::Effects`]: crate::actions::ActionResult::Effects
//! [`AppEffect::Pending`]: crate::actions::effects::AppEffect::Pending

pub use xeno_primitives::PendingKind;

/// State for actions waiting on additional user input.
///
/// Created by [`AppEffect::Pending`] to signal that the editor should
/// capture more input before completing the action (e.g., `f` needs a char).
///
/// [`AppEffect::Pending`]: crate::actions::effects::AppEffect::Pending
#[derive(Debug, Clone)]
pub struct PendingAction {
	/// What type of input is expected.
	pub kind: PendingKind,
	/// Prompt to display while waiting.
	pub prompt: String,
}
