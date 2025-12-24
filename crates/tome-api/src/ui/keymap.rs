use std::collections::HashMap;

use termina::event::KeyEvent;
use tome_core::{Key, KeyCode, Modifiers};

use super::UiRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiKeyChord(pub Key);

impl UiKeyChord {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindingScope {
	Global,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
	pub scope: BindingScope,
	pub chord: UiKeyChord,
	pub priority: i16,
	pub requests: Vec<UiRequest>,
}

#[derive(Debug, Default)]
pub struct KeybindingRegistry {
	bindings: Vec<Keybinding>,
	index: HashMap<(BindingScope, UiKeyChord), Vec<usize>>,
}

impl KeybindingRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn register(&mut self, binding: Keybinding) {
		let idx = self.bindings.len();
		let key = (binding.scope.clone(), binding.chord);
		self.bindings.push(binding);
		self.index.entry(key).or_default().push(idx);
	}

	pub fn register_global(&mut self, chord: UiKeyChord, priority: i16, requests: Vec<UiRequest>) {
		self.register(Keybinding {
			scope: BindingScope::Global,
			chord,
			priority,
			requests,
		});
	}

	pub fn match_key(&self, scope: &BindingScope, key: &KeyEvent) -> Option<&Keybinding> {
		let chord = UiKeyChord::from(key);
		let indices = self.index.get(&(scope.clone(), chord))?;
		indices
			.iter()
			.filter_map(|i| self.bindings.get(*i))
			.min_by_key(|b| b.priority)
	}
}
