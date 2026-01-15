//! Input handler managing key processing and mode state.

use tracing::debug;
use xeno_keymap_core::ToKeyMap;
use xeno_keymap_core::parser::Node;
use xeno_primitives::key::{Key, KeyCode, MouseButton, MouseEvent};
use xeno_registry::keymap_registry::{KeymapRegistry, LookupResult};
use xeno_registry::{BindingMode, get_keymap_registry};

use super::types::{KeyResult, Mode};

/// Manages input state and key processing.
#[derive(Debug, Clone)]
pub struct InputHandler {
	/// Current editor mode (normal, insert, window, pending action).
	pub(crate) mode: Mode,
	/// Accumulated count prefix for actions (e.g., `3j` to move down 3 lines).
	pub(crate) count: u32,
	/// Selected register for yank/paste operations.
	pub(crate) register: Option<char>,
	/// For commands that extend selection.
	pub(crate) extend: bool,
	/// Last search pattern for n/N repeat.
	pub(crate) last_search: Option<(String, bool)>,
	/// Accumulated key sequence for multi-key bindings (e.g., `g g`).
	pub(crate) key_sequence: Vec<Node>,
}

impl Default for InputHandler {
	fn default() -> Self {
		Self::new()
	}
}

impl InputHandler {
	/// Creates a new input handler in normal mode with default state.
	pub fn new() -> Self {
		Self {
			mode: Mode::Normal,
			count: 0,
			register: None,
			extend: false,
			last_search: None,
			key_sequence: Vec::new(),
		}
	}

	/// Returns the current editor mode.
	pub fn mode(&self) -> Mode {
		self.mode.clone()
	}

	/// Returns the display name for the current mode.
	pub fn mode_name(&self) -> &'static str {
		use xeno_primitives::PendingKind;
		match &self.mode {
			Mode::Normal => "NORMAL",
			Mode::Insert => "INSERT",
			Mode::PendingAction(kind) => match kind {
				PendingKind::FindChar { .. } | PendingKind::FindCharReverse { .. } => "FIND",
				PendingKind::ReplaceChar => "REPLACE",
				PendingKind::Object(_) => "OBJECT",
			},
		}
	}

	/// Returns the accumulated count prefix, or 0 if none.
	pub fn count(&self) -> u32 {
		self.count
	}

	/// Returns the count prefix, defaulting to 1 if not specified.
	pub fn effective_count(&self) -> u32 {
		if self.count == 0 { 1 } else { self.count }
	}

	/// Returns the selected register, if any.
	pub fn register(&self) -> Option<char> {
		self.register
	}

	/// Sets the editor mode, resetting parameters when entering normal mode.
	pub fn set_mode(&mut self, mode: Mode) {
		self.mode = mode.clone();
		if matches!(mode, Mode::Normal) {
			self.reset_params();
		}
	}

	/// Stores the last search pattern and direction for repeat commands.
	pub fn set_last_search(&mut self, pattern: String, reverse: bool) {
		self.last_search = Some((pattern, reverse));
	}

	/// Returns the last search pattern and direction, if any.
	pub fn last_search(&self) -> Option<(&str, bool)> {
		self.last_search.as_ref().map(|(p, r)| (p.as_str(), *r))
	}

	/// Resets count, register, extend, and key sequence to defaults.
	pub(crate) fn reset_params(&mut self) {
		self.count = 0;
		self.register = None;
		self.extend = false;
		self.key_sequence.clear();
	}

	/// Returns the number of keys in the pending sequence.
	pub fn pending_key_count(&self) -> usize {
		self.key_sequence.len()
	}

	/// Returns the pending key sequence for display (e.g., which-key HUD).
	pub fn pending_keys(&self) -> &[Node] {
		&self.key_sequence
	}

	/// Clears the pending key sequence.
	pub fn clear_key_sequence(&mut self) {
		self.key_sequence.clear();
	}

	/// Process a key and return the result.
	pub fn handle_key(&mut self, key: Key) -> KeyResult {
		let registry = get_keymap_registry();

		match &self.mode {
			Mode::Normal => self.handle_mode_key(key, BindingMode::Normal, registry),
			Mode::Insert => self.handle_insert_key(key),
			Mode::PendingAction(kind) => {
				let kind = *kind;
				self.handle_pending_action_key(key, kind)
			}
		}
	}

	/// Handles a key in a specific binding mode (normal, window, etc.).
	fn handle_mode_key(
		&mut self,
		key: Key,
		binding_mode: BindingMode,
		registry: &KeymapRegistry,
	) -> KeyResult {
		if binding_mode == BindingMode::Normal
			&& let Some(digit) = key.as_digit()
			&& (digit != 0 || self.count > 0)
		{
			self.count = self.count.saturating_mul(10).saturating_add(digit);
			return KeyResult::Consumed;
		}

		if key.is_escape() {
			if !self.key_sequence.is_empty() {
				self.key_sequence.clear();
				return KeyResult::Consumed;
			}
			if binding_mode != BindingMode::Normal {
				self.mode = Mode::Normal;
				self.reset_params();
				return KeyResult::ModeChange(Mode::Normal);
			}
			self.reset_params();
			return KeyResult::Consumed;
		}

		let key = self.process_shift_extend(key);

		let Ok(node) = key.to_keymap() else {
			self.reset_params();
			return KeyResult::Unhandled;
		};

		self.key_sequence.push(node.clone());

		let lookup_result = registry.lookup(binding_mode, &self.key_sequence);

		let lookup_result = match (&lookup_result, key.code) {
			(LookupResult::None, KeyCode::Char(c)) if c.is_ascii_uppercase() => {
				self.key_sequence.pop();
				let lowercase_key = Key {
					code: KeyCode::Char(c.to_ascii_lowercase()),
					modifiers: key.modifiers,
				};
				if let Ok(lowercase_node) = lowercase_key.to_keymap() {
					self.key_sequence.push(lowercase_node);
					registry.lookup(binding_mode, &self.key_sequence)
				} else {
					lookup_result
				}
			}
			_ => lookup_result,
		};

		match lookup_result {
			LookupResult::Match(entry) => {
				let count = if self.count > 0 {
					self.count as usize
				} else {
					1
				};
				let extend = self.extend;
				let register = self.register;
				let action_id = entry.action_id;

				if binding_mode != BindingMode::Normal {
					self.mode = Mode::Normal;
				}
				self.reset_params();

				KeyResult::ActionById {
					id: action_id,
					count,
					extend,
					register,
				}
			}
			LookupResult::Pending { sticky } => {
				if let Some(entry) = sticky {
					debug!(
						action = entry.action_name,
						keys = self.key_sequence.len(),
						"Pending with sticky action"
					);
				}
				KeyResult::Pending {
					keys_so_far: self.key_sequence.len(),
				}
			}
			LookupResult::None => {
				if binding_mode != BindingMode::Normal {
					self.mode = Mode::Normal;
				}
				self.reset_params();
				KeyResult::Unhandled
			}
		}
	}

	/// Processes shift modifier to set extend flag and normalize the key.
	fn process_shift_extend(&mut self, key: Key) -> Key {
		if let KeyCode::Char(c) = key.code
			&& c.is_ascii_uppercase()
		{
			if key.modifiers.shift {
				self.extend = true;
			}
			return key.drop_shift();
		}

		if key.modifiers.shift {
			self.extend = true;
			return key.drop_shift();
		}

		key
	}

	/// Processes a mouse event and returns the appropriate result.
	pub fn handle_mouse(&mut self, event: MouseEvent) -> KeyResult {
		match event {
			MouseEvent::Press {
				button: MouseButton::Left,
				row,
				col,
				..
			} => KeyResult::MouseClick {
				row,
				col,
				extend: self.extend,
			},
			MouseEvent::Drag {
				button: MouseButton::Left,
				row,
				col,
				..
			} => KeyResult::MouseDrag { row, col },
			MouseEvent::Scroll { direction, .. } => KeyResult::MouseScroll {
				direction,
				count: 1,
			},
			_ => KeyResult::Consumed,
		}
	}
}
