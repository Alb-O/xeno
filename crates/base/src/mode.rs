//!
//! Editor mode state.

use crate::PendingKind;

/// Editor mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
	#[default]
	Normal,
	Insert,
	/// Window/split management mode (Ctrl+w prefix).
	Window,
	/// Waiting for character input to complete an action.
	PendingAction(PendingKind),
}

impl Mode {
	/// Returns a simple string identifier for the mode.
	pub fn name(&self) -> &'static str {
		match self {
			Mode::Normal => "normal",
			Mode::Insert => "insert",
			Mode::Window => "window",
			Mode::PendingAction(_) => "pending",
		}
	}
}
