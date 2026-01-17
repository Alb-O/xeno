//! Keybindings map key sequences to actions in different modes.
//!
//! Uses a trie-based registry for efficient sequence matching (e.g., `g g` for
//! document_start). Keybindings are colocated with their action definitions using
//! the `action!` macro's `bindings:` syntax.

use std::sync::LazyLock;

use xeno_primitives::Mode;

/// Registry wrapper for keybinding sets.
pub struct KeyBindingSetReg(pub &'static [KeyBindingDef]);
inventory::collect!(KeyBindingSetReg);

/// Registry wrapper for key prefix definitions.
pub struct KeyPrefixReg(pub &'static KeyPrefixDef);
inventory::collect!(KeyPrefixReg);

/// All keybindings from `action!` macro `bindings:` syntax.
pub static KEYBINDINGS: LazyLock<Vec<KeyBindingDef>> = LazyLock::new(|| {
	let mut bindings = Vec::new();
	for set in inventory::iter::<KeyBindingSetReg> {
		bindings.extend_from_slice(set.0);
	}
	bindings
});

/// All key prefixes from `key_prefix!` macro.
///
/// Used by the which-key HUD to show a description for the pressed prefix key.
pub static KEY_PREFIXES: LazyLock<Vec<&'static KeyPrefixDef>> =
	LazyLock::new(|| inventory::iter::<KeyPrefixReg>().map(|r| r.0).collect());

/// Definition of a key sequence prefix with its description.
///
/// Registered via the `key_prefix!` macro:
///
/// ```ignore
/// key_prefix!(normal "g" => "Goto");
/// key_prefix!(normal "z" => "View");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct KeyPrefixDef {
	/// Mode this prefix is active in.
	pub mode: BindingMode,
	/// The prefix key sequence (e.g., `"g"`, `"z"`).
	pub keys: &'static str,
	/// Human-readable description (e.g., "Goto", "View").
	pub description: &'static str,
}

/// Finds a prefix definition for the given mode and key sequence.
pub fn find_prefix(mode: BindingMode, keys: &str) -> Option<&'static KeyPrefixDef> {
	KEY_PREFIXES
		.iter()
		.copied()
		.find(|p| p.mode == mode && p.keys == keys)
}

/// Key sequence binding definition.
///
/// Maps a key sequence (e.g., `"g g"`, `"ctrl-home"`) to an action in a mode.
#[derive(Clone, Copy)]
pub struct KeyBindingDef {
	/// Mode this binding is active in.
	pub mode: BindingMode,
	/// Key sequence string (e.g., `"g g"`, `"ctrl-home"`).
	/// Parsed with `parse_seq()` at registry initialization.
	pub keys: &'static str,
	/// Action to execute (looked up by name in the action registry).
	pub action: &'static str,
	/// Priority for conflict resolution (lower wins).
	/// Default bindings use 100; user overrides should use lower values.
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
