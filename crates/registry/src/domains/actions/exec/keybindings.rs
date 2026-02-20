//! Keybindings map key sequences to actions in different modes.

use std::sync::Arc;

use xeno_primitives::Mode;

/// Key sequence binding definition.
#[derive(Clone)]
pub struct KeyBindingDef {
	/// Mode this binding is active in.
	pub mode: BindingMode,
	/// Key sequence string (e.g., `"g g"`, `"ctrl-home"`).
	pub keys: Arc<str>,
	/// Action to execute (looked up by name in the action registry).
	pub action: Arc<str>,
	/// Priority for conflict resolution (lower wins).
	pub priority: i16,
}

impl std::fmt::Debug for KeyBindingDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("KeyBindingDef")
			.field("mode", &self.mode)
			.field("keys", &self.keys)
			.field("action", &self.action)
			.field("priority", &self.priority)
			.finish()
	}
}

/// Mode in which a keybinding is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BindingMode {
	/// Normal mode (default editing mode).
	Normal,
	/// Insert mode (text input).
	Insert,
	/// Match mode (m prefix).
	Match,
	/// Space mode (space prefix).
	Space,
}

impl From<Mode> for BindingMode {
	fn from(mode: Mode) -> Self {
		match mode {
			Mode::Normal => BindingMode::Normal,
			Mode::Insert => BindingMode::Insert,
			Mode::PendingAction(_) => BindingMode::Normal,
		}
	}
}
