//! Keybindings map key sequences to actions in different modes.

use std::sync::{Arc, LazyLock};

use xeno_primitives::Mode;

#[cfg(feature = "db")]
use crate::db::get_db;

/// Key prefixes extracted from the registry database.
pub static KEY_PREFIXES: LazyLock<&'static [KeyPrefixDef]> = LazyLock::new(current_key_prefixes);

fn current_key_prefixes() -> &'static [KeyPrefixDef] {
	#[cfg(feature = "db")]
	{
		get_db().key_prefixes.as_slice()
	}

	#[cfg(not(feature = "db"))]
	{
		&[]
	}
}

/// Definition of a key sequence prefix with its description.
#[derive(Debug, Clone)]
pub struct KeyPrefixDef {
	/// Mode this prefix is active in.
	pub mode: BindingMode,
	/// The prefix key sequence (e.g., `"g"`, `"z"`).
	pub keys: Arc<str>,
	/// Human-readable description (e.g., "Goto", "View").
	pub description: Arc<str>,
}

/// Finds a prefix definition for the given mode and key sequence.
pub fn find_prefix(mode: BindingMode, keys: &str) -> Option<&'static KeyPrefixDef> {
	KEY_PREFIXES.iter().find(|p| p.mode == mode && &*p.keys == keys)
}

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
