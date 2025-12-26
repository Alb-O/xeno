//! Editor mode definitions.

use crate::PendingKind;

/// Editor mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
	#[default]
	Normal,
	Insert,
	Goto,
	View,
	/// Waiting for character input to complete an action.
	PendingAction(PendingKind),
}

impl Mode {
	/// Returns a simple string identifier for the mode.
	pub fn name(&self) -> &'static str {
		match self {
			Mode::Normal => "normal",
			Mode::Insert => "insert",
			Mode::Goto => "goto",
			Mode::View => "view",
			Mode::PendingAction(_) => "pending",
		}
	}
}
