//! Action result type.
//!
//! The [`ActionResult`] enum is the return type for all action handlers,
//! describing what the editor should do after an action executes.

use crate::actions::effects::ActionEffects;

/// Screen-relative cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenPosition {
	/// First visible line (vim H).
	Top,
	/// Middle visible line (vim M).
	Middle,
	/// Last visible line (vim L).
	Bottom,
}

/// Result of executing an action.
///
/// All actions return `ActionResult::Effects(...)` containing composable
/// primitive effects. The `apply_effects` function processes these effects
/// to mutate editor state.
#[derive(Debug, Clone)]
pub enum ActionResult {
	/// Apply a set of composable effects.
	///
	/// This is the sole variant - all editor state changes are expressed
	/// as compositions of primitive [`Effect`](crate::actions::Effect) values.
	Effects(ActionEffects),
}

impl ActionResult {
	/// Returns the variant name as a static string.
	///
	/// Used for hook events and debugging.
	pub fn variant_name(&self) -> &'static str {
		match self {
			ActionResult::Effects(..) => "Effects",
		}
	}
}
