//! Keybindings map key sequences to actions in different modes.

use std::sync::LazyLock;

use inventory;
use xeno_primitives::Mode;

use crate::inventory::Reg;

/// All keybindings from `action!` macro `bindings:` syntax.
pub static KEYBINDINGS: LazyLock<Vec<KeyBindingDef>> = LazyLock::new(|| {
	let mut bindings = Vec::new();
	for set in inventory::iter::<crate::inventory::RegSlice<KeyBindingDef>>.into_iter() {
		bindings.extend_from_slice(set.0);
	}
	bindings
});

/// All key prefixes from `key_prefix!` macro.
pub static KEY_PREFIXES: LazyLock<Vec<&'static KeyPrefixDef>> = LazyLock::new(|| {
	inventory::iter::<Reg<KeyPrefixDef>>
		.into_iter()
		.map(|r| r.0)
		.collect()
});

/// Definition of a key sequence prefix with its description.
#[derive(Debug, Clone, Copy)]
pub struct KeyPrefixDef {
	/// Mode this prefix is active in.
	pub mode: BindingMode,
	/// The prefix key sequence (e.g., `"g"`, `"z"`).
	pub keys: &'static str,
	/// Human-readable description (e.g., "Goto", "View").
	pub description: &'static str,
}

// Manually collect here to ensure visibility
inventory::collect!(Reg<KeyPrefixDef>);

/// Finds a prefix definition for the given mode and key sequence.
pub fn find_prefix(mode: BindingMode, keys: &str) -> Option<&'static KeyPrefixDef> {
	KEY_PREFIXES
		.iter()
		.copied()
		.find(|p| p.mode == mode && p.keys == keys)
}

/// Key sequence binding definition.
#[derive(Clone, Copy)]
pub struct KeyBindingDef {
	/// Mode this binding is active in.
	pub mode: BindingMode,
	/// Key sequence string (e.g., `"g g"`, `"ctrl-home"`).
	pub keys: &'static str,
	/// Action to execute (looked up by name in the action registry).
	pub action: &'static str,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
