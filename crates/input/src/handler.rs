//! Input handler managing key processing and mode state.

use evildoer_base::key::{Key, KeyCode, MouseButton, MouseEvent};
use evildoer_keymap::ToKeyMap;
use evildoer_keymap::parser::Node;
use evildoer_manifest::keymap_registry::{KeymapRegistry, LookupResult};
use evildoer_manifest::{BindingMode, get_keymap_registry};
use tracing::debug;

use crate::types::{KeyResult, Mode};

/// Manages input state and key processing.
#[derive(Debug, Clone)]
pub struct InputHandler {
	pub(crate) mode: Mode,
	pub(crate) count: u32,
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

	pub fn mode(&self) -> Mode {
		self.mode.clone()
	}

	pub fn mode_name(&self) -> &'static str {
		use evildoer_manifest::PendingKind;
		match &self.mode {
			Mode::Normal => "NORMAL",
			Mode::Insert => "INSERT",
			Mode::Window => "WINDOW",
			Mode::PendingAction(kind) => match kind {
				PendingKind::FindChar { .. } | PendingKind::FindCharReverse { .. } => "FIND",
				PendingKind::ReplaceChar => "REPLACE",
				PendingKind::Object(_) => "OBJECT",
			},
		}
	}

	pub fn count(&self) -> u32 {
		self.count
	}

	pub fn effective_count(&self) -> u32 {
		if self.count == 0 { 1 } else { self.count }
	}

	pub fn register(&self) -> Option<char> {
		self.register
	}

	pub fn set_mode(&mut self, mode: Mode) {
		self.mode = mode.clone();
		if matches!(mode, Mode::Normal) {
			self.reset_params();
		}
	}

	pub fn set_last_search(&mut self, pattern: String, reverse: bool) {
		self.last_search = Some((pattern, reverse));
	}

	pub fn last_search(&self) -> Option<(&str, bool)> {
		self.last_search.as_ref().map(|(p, r)| (p.as_str(), *r))
	}

	pub(crate) fn reset_params(&mut self) {
		self.count = 0;
		self.register = None;
		self.extend = false;
		self.key_sequence.clear();
	}

	pub fn pending_key_count(&self) -> usize {
		self.key_sequence.len()
	}

	pub fn clear_key_sequence(&mut self) {
		self.key_sequence.clear();
	}

	/// Process a key and return the result.
	pub fn handle_key(&mut self, key: Key) -> KeyResult {
		let registry = get_keymap_registry();

		match &self.mode {
			Mode::Normal => self.handle_mode_key(key, BindingMode::Normal, registry),
			Mode::Insert => self.handle_insert_key(key),
			Mode::Window => self.handle_mode_key(key, BindingMode::Window, registry),
			Mode::PendingAction(kind) => {
				let kind = *kind;
				self.handle_pending_action_key(key, kind)
			}
		}
	}

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
