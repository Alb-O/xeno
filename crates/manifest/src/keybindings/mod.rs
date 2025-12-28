//! Keybinding registration system.
//!
//! Keybindings map keys to actions in different modes. This replaces
//! the hardcoded keymap arrays with an extensible registry.
//!
//! All keybindings are now colocated with their action definitions using
//! the `bound_action!` macro with `bindings:` syntax. For example:
//!
//! ```ignore
//! bound_action!(
//!     document_start,
//!     description: "Move to document start",
//!     bindings: [
//!         Normal => [Key::special(SpecialKey::Home).with_ctrl()],
//!         Goto => [Key::char('g'), Key::char('k')],
//!         Insert => [Key::special(SpecialKey::Home).with_ctrl()],
//!     ],
//!     |_ctx| { ... }
//! );
//! ```

use linkme::distributed_slice;
use tome_base::key::Key;

use crate::index::resolve_action_id;
use crate::{ActionId, Mode};

macro_rules! keybinding_slices {
    ($($slice:ident),+ $(,)?) => {
        $(#[distributed_slice]
        pub static $slice: [KeyBindingDef];)+
    };
}

keybinding_slices!(
	KEYBINDINGS_NORMAL,
	KEYBINDINGS_INSERT,
	KEYBINDINGS_GOTO,
	KEYBINDINGS_VIEW,
	KEYBINDINGS_MATCH,
	KEYBINDINGS_WINDOW,
	KEYBINDINGS_SPACE,
);

/// A keybinding definition that maps a key to an action in a specific mode.
#[derive(Clone, Copy)]
pub struct KeyBindingDef {
	/// The mode this binding is active in.
	pub mode: BindingMode,
	/// The key that triggers this binding.
	pub key: Key,
	/// The action to execute (by name).
	pub action: &'static str,
	/// Priority for conflict resolution (lower = higher priority).
	/// Default bindings use 100, user overrides should use lower values.
	pub priority: i16,
}

impl std::fmt::Debug for KeyBindingDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("KeyBindingDef")
			.field("mode", &self.mode)
			.field("key", &self.key)
			.field("action", &self.action)
			.field("priority", &self.priority)
			.finish()
	}
}

/// The mode a keybinding applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMode {
	/// Normal mode (default editing mode).
	Normal,
	/// Insert mode (text input).
	Insert,
	/// Goto mode (g prefix).
	Goto,
	/// View mode (z prefix).
	View,
	/// Match mode (m prefix).
	Match,
	/// Window mode (Ctrl+w prefix).
	Window,
	/// Space mode (space prefix).
	Space,
}

impl From<Mode> for BindingMode {
	fn from(mode: Mode) -> Self {
		match mode {
			Mode::Normal => BindingMode::Normal,
			Mode::Insert => BindingMode::Insert,
			Mode::Goto => BindingMode::Goto,
			Mode::View => BindingMode::View,
			Mode::Window => BindingMode::Window,
			Mode::PendingAction(_) => BindingMode::Normal,
		}
	}
}

fn slice_for_mode(mode: BindingMode) -> &'static [KeyBindingDef] {
	match mode {
		BindingMode::Normal => &KEYBINDINGS_NORMAL,
		BindingMode::Insert => &KEYBINDINGS_INSERT,
		BindingMode::Goto => &KEYBINDINGS_GOTO,
		BindingMode::View => &KEYBINDINGS_VIEW,
		BindingMode::Match => &KEYBINDINGS_MATCH,
		BindingMode::Window => &KEYBINDINGS_WINDOW,
		BindingMode::Space => &KEYBINDINGS_SPACE,
	}
}

fn all_slices() -> impl Iterator<Item = &'static KeyBindingDef> {
	KEYBINDINGS_NORMAL
		.iter()
		.chain(KEYBINDINGS_INSERT.iter())
		.chain(KEYBINDINGS_GOTO.iter())
		.chain(KEYBINDINGS_VIEW.iter())
		.chain(KEYBINDINGS_MATCH.iter())
		.chain(KEYBINDINGS_WINDOW.iter())
		.chain(KEYBINDINGS_SPACE.iter())
}

/// Look up a keybinding for the given mode and key.
/// Returns the highest-priority (lowest value) binding if multiple match.
pub fn find_binding(mode: BindingMode, key: Key) -> Option<&'static KeyBindingDef> {
	slice_for_mode(mode)
		.iter()
		.filter(|kb| kb.key == key)
		.min_by_key(|kb| kb.priority)
}

/// Resolved keybinding with typed ActionId.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedBinding {
	/// The keybinding definition.
	pub binding: &'static KeyBindingDef,
	/// The resolved ActionId for efficient dispatch.
	pub action_id: ActionId,
}

/// Look up a keybinding and resolve its action to a typed ActionId.
/// This is the preferred method for input handling as it enables type-safe dispatch.
///
/// Returns None if no binding exists for the mode/key, or if the action name
/// cannot be resolved to an ActionId (which indicates a configuration error).
pub fn find_binding_resolved(mode: BindingMode, key: Key) -> Option<ResolvedBinding> {
	let binding = find_binding(mode, key)?;
	let action_id = resolve_action_id(binding.action)?;
	Some(ResolvedBinding { binding, action_id })
}

/// Get all keybindings for a specific mode.
pub fn bindings_for_mode(mode: BindingMode) -> impl Iterator<Item = &'static KeyBindingDef> {
	slice_for_mode(mode).iter()
}

/// Get all keybindings for a specific action.
pub fn bindings_for_action(action: &str) -> impl Iterator<Item = &'static KeyBindingDef> {
	all_slices().filter(move |kb| kb.action == action)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_binding_mode_from_mode() {
		assert_eq!(BindingMode::from(Mode::Normal), BindingMode::Normal);
		assert_eq!(BindingMode::from(Mode::Insert), BindingMode::Insert);
		assert_eq!(BindingMode::from(Mode::Goto), BindingMode::Goto);
	}
}
