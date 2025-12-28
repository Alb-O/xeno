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
use evildoer_base::key::Key;

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

/// Keybinding definition mapping a [`Key`] to an action in a [`BindingMode`].
///
/// Registered at compile time via [`linkme`] distributed slices, typically
/// using the `bindings:` syntax in [`bound_action!`](crate::bound_action).
#[derive(Clone, Copy)]
pub struct KeyBindingDef {
	/// Mode this binding is active in.
	pub mode: BindingMode,
	/// Key that triggers this binding.
	pub key: Key,
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
			.field("key", &self.key)
			.field("action", &self.action)
			.field("priority", &self.priority)
			.finish()
	}
}

/// Mode in which a keybinding is active.
///
/// Each mode has its own distributed slice of [`KeyBindingDef`] entries.
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

/// Finds the keybinding for `key` in `mode`.
///
/// Returns the highest-priority binding (lowest priority value) if multiple
/// bindings match.
pub fn find_binding(mode: BindingMode, key: Key) -> Option<&'static KeyBindingDef> {
	slice_for_mode(mode)
		.iter()
		.filter(|kb| kb.key == key)
		.min_by_key(|kb| kb.priority)
}

/// Keybinding with its action resolved to a typed [`ActionId`].
///
/// Returned by [`find_binding_resolved`] for efficient dispatch without
/// repeated string lookups.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedBinding {
	/// Original keybinding definition.
	pub binding: &'static KeyBindingDef,
	/// Resolved action ID for type-safe dispatch.
	pub action_id: ActionId,
}

/// Finds and resolves a keybinding to a typed [`ActionId`].
///
/// Preferred method for input handling as it enables type-safe dispatch.
/// Returns [`None`] if no binding exists or if the action name cannot be
/// resolved (indicating a configuration error).
pub fn find_binding_resolved(mode: BindingMode, key: Key) -> Option<ResolvedBinding> {
	let binding = find_binding(mode, key)?;
	let action_id = resolve_action_id(binding.action)?;
	Some(ResolvedBinding { binding, action_id })
}

/// Returns all keybindings registered for `mode`.
pub fn bindings_for_mode(mode: BindingMode) -> impl Iterator<Item = &'static KeyBindingDef> {
	slice_for_mode(mode).iter()
}

/// Returns all keybindings that trigger `action` (across all modes).
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
