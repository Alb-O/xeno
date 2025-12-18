//! Keybinding registration system.
//!
//! Keybindings map keys to actions in different modes. This replaces
//! the hardcoded keymap arrays with an extensible registry.

mod goto;
mod insert;
mod normal;
mod view;

use linkme::distributed_slice;

use crate::Mode;
use crate::key::Key;

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
			Mode::Command { .. } => BindingMode::Normal,
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

	#[test]
	fn test_normal_mode_bindings_registered() {
		let h = find_binding(BindingMode::Normal, Key::char('h'));
		assert!(h.is_some());
		assert_eq!(h.unwrap().action, "move_left");

		let l = find_binding(BindingMode::Normal, Key::char('l'));
		assert!(l.is_some());
		assert_eq!(l.unwrap().action, "move_right");

		let w = find_binding(BindingMode::Normal, Key::char('w'));
		assert!(w.is_some());
		assert_eq!(w.unwrap().action, "next_word_start");
	}

	#[test]
	fn test_goto_mode_bindings_registered() {
		let g = find_binding(BindingMode::Goto, Key::char('g'));
		assert!(g.is_some());
		assert_eq!(g.unwrap().action, "document_start");

		let e = find_binding(BindingMode::Goto, Key::char('e'));
		assert!(e.is_some());
		assert_eq!(e.unwrap().action, "document_end");

		let h = find_binding(BindingMode::Goto, Key::char('h'));
		assert!(h.is_some());
		assert_eq!(h.unwrap().action, "move_line_start");
	}

	#[test]
	fn test_view_mode_bindings_registered() {
		let c = find_binding(BindingMode::View, Key::char('c'));
		assert!(c.is_some());
		assert_eq!(c.unwrap().action, "center_cursor");

		let j = find_binding(BindingMode::View, Key::char('j'));
		assert!(j.is_some());
		assert_eq!(j.unwrap().action, "scroll_down");
	}

	#[test]
	fn test_bindings_for_mode() {
		let normal_bindings: Vec<_> = bindings_for_mode(BindingMode::Normal).collect();
		assert!(normal_bindings.len() >= 10);

		let goto_bindings: Vec<_> = bindings_for_mode(BindingMode::Goto).collect();
		assert!(goto_bindings.len() >= 5);

		let view_bindings: Vec<_> = bindings_for_mode(BindingMode::View).collect();
		assert!(view_bindings.len() >= 4);

		let insert_bindings: Vec<_> = bindings_for_mode(BindingMode::Insert).collect();
		assert!(
			insert_bindings.len() >= 6,
			"should have insert mode bindings"
		);
	}
}
