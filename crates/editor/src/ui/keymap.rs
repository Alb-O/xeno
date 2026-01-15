//! UI keybinding registry for panel and global shortcuts.

use std::collections::HashMap;

use termina::event::KeyEvent;
use xeno_primitives::{Key, KeyCode, Modifiers};

use super::UiRequest;

/// A single key chord for UI bindings, wrapping an internal Key type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiKeyChord(pub Key);

impl UiKeyChord {
	/// Creates a chord for Ctrl+char.
	pub const fn ctrl_char(c: char) -> Self {
		Self(Key {
			code: KeyCode::Char(c),
			modifiers: Modifiers::CTRL,
		})
	}
}

impl From<&KeyEvent> for UiKeyChord {
	fn from(value: &KeyEvent) -> Self {
		let key: Key = (*value).into();
		Self(key)
	}
}

/// Scope in which a keybinding is active.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindingScope {
	/// Binding active regardless of current focus.
	Global,
}

/// A keybinding that triggers UI requests.
#[derive(Debug, Clone)]
pub struct Keybinding {
	/// The scope in which this binding is active.
	pub scope: BindingScope,
	/// The key chord that triggers this binding.
	pub chord: UiKeyChord,
	/// Priority for conflict resolution (lower wins).
	pub priority: i16,
	/// UI requests to execute when triggered.
	pub requests: Vec<UiRequest>,
}

/// Registry of UI keybindings indexed by scope and chord.
#[derive(Debug, Default)]
pub struct KeybindingRegistry {
	/// All registered bindings.
	bindings: Vec<Keybinding>,
	/// Index from (scope, chord) to binding indices for fast lookup.
	index: HashMap<(BindingScope, UiKeyChord), Vec<usize>>,
}

impl KeybindingRegistry {
	/// Creates an empty keybinding registry.
	pub fn new() -> Self {
		Self::default()
	}

	/// Registers a keybinding in the registry.
	pub fn register(&mut self, binding: Keybinding) {
		let idx = self.bindings.len();
		let key = (binding.scope.clone(), binding.chord);
		self.bindings.push(binding);
		self.index.entry(key).or_default().push(idx);
	}

	/// Registers a global keybinding with the given chord, priority, and requests.
	pub fn register_global(&mut self, chord: UiKeyChord, priority: i16, requests: Vec<UiRequest>) {
		self.register(Keybinding {
			scope: BindingScope::Global,
			chord,
			priority,
			requests,
		});
	}

	/// Finds the highest priority binding for the given scope and key event.
	pub fn match_key(&self, scope: &BindingScope, key: &KeyEvent) -> Option<&Keybinding> {
		let chord = UiKeyChord::from(key);
		let indices = self.index.get(&(scope.clone(), chord))?;
		indices
			.iter()
			.filter_map(|i| self.bindings.get(*i))
			.min_by_key(|b| b.priority)
	}
}
