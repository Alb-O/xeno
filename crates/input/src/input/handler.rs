//! Input handler managing key processing and mode state.

use tracing::debug;
use xeno_keymap_core::ToKeyMap;
use xeno_keymap_core::parser::Node;
use xeno_primitives::key::{Key, KeyCode, MouseButton, MouseEvent};
use xeno_registry::actions::BindingMode;
use xeno_registry::{KeymapIndex, LookupResult, get_keymap_registry};

use super::types::{KeyResult, Mode};

/// Modal input state machine that resolves key sequences against the keymap registry.
///
/// Tracks mode, count prefix, register, extend flag, and multi-key sequences.
/// Stateless with respect to editor — returns [`KeyResult`] values that the
/// editor layer interprets.
#[derive(Debug, Clone)]
pub struct InputHandler {
	pub(crate) mode: Mode,
	pub(crate) count: u32,
	pub(crate) register: Option<char>,
	pub(crate) extend: bool,
	pub(crate) last_search: Option<(String, bool)>,
	pub(crate) key_sequence: Vec<Node>,
}

impl Default for InputHandler {
	fn default() -> Self {
		Self::new()
	}
}

impl InputHandler {
	/// Creates a new handler in normal mode.
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

	/// Returns a short display label for the current mode.
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

	/// Returns the raw count prefix (0 means unset).
	pub fn count(&self) -> u32 {
		self.count
	}

	/// Returns the count prefix, treating unset as 1.
	pub fn effective_count(&self) -> u32 {
		self.count.max(1)
	}

	/// Returns the selected register.
	pub fn register(&self) -> Option<char> {
		self.register
	}

	/// Sets the mode, resetting count/register/extend when entering normal.
	pub fn set_mode(&mut self, mode: Mode) {
		if matches!(mode, Mode::Normal) {
			self.reset_params();
		}
		self.mode = mode;
	}

	/// Stores the last search pattern and direction for `n`/`N` repeat.
	pub fn set_last_search(&mut self, pattern: String, reverse: bool) {
		self.last_search = Some((pattern, reverse));
	}

	/// Returns the last search `(pattern, reverse)` pair.
	pub fn last_search(&self) -> Option<(&str, bool)> {
		self.last_search.as_ref().map(|(p, r)| (p.as_str(), *r))
	}

	/// Consumes the current count/extend/register state into an [`ActionById`] result.
	pub(crate) fn consume_action(&mut self, id: xeno_registry::ActionId) -> KeyResult {
		let count = self.effective_count() as usize;
		let extend = self.extend;
		let register = self.register;
		self.reset_params();
		KeyResult::ActionById { id, count, extend, register }
	}

	/// Resets transient key-processing state to defaults.
	pub(crate) fn reset_params(&mut self) {
		self.count = 0;
		self.register = None;
		self.extend = false;
		self.key_sequence.clear();
	}

	/// Returns the number of keys accumulated in the pending sequence.
	pub fn pending_key_count(&self) -> usize {
		self.key_sequence.len()
	}

	/// Returns the pending key sequence for which-key display.
	pub fn pending_keys(&self) -> &[Node] {
		&self.key_sequence
	}

	/// Clears the pending key sequence.
	pub fn clear_key_sequence(&mut self) {
		self.key_sequence.clear();
	}

	/// Dispatches a key through the current mode's handler.
	pub fn handle_key(&mut self, key: Key) -> KeyResult {
		let registry = get_keymap_registry();
		self.handle_key_with_registry(key, &registry)
	}

	/// Dispatches a key using an explicit keymap registry.
	pub fn handle_key_with_registry(&mut self, key: Key, registry: &KeymapIndex) -> KeyResult {
		match &self.mode {
			Mode::Normal => self.handle_mode_key(key, BindingMode::Normal, registry),
			Mode::Insert => self.handle_insert_key(key, registry),
			Mode::PendingAction(kind) => {
				let kind = *kind;
				self.handle_pending_action_key(key, kind)
			}
		}
	}

	/// Resolves a key against a binding mode's keymap.
	///
	/// Shift+alphabetic keys are canonicalized to uppercase-without-shift
	/// (Vim semantics: Shift+n looks up "N"). If the uppercase lookup fails,
	/// the lowercase variant is tried with `extend = true` (Shift extends
	/// selection). Non-alphabetic Shift keys follow the same extend fallback.
	fn handle_mode_key(&mut self, key: Key, binding_mode: BindingMode, registry: &KeymapIndex) -> KeyResult {
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

		let lookup_with = |k: Key, seq: &mut Vec<Node>| -> Option<LookupResult> {
			let node = k.to_keymap().ok()?;
			seq.push(node);
			let res = registry.lookup(binding_mode, seq);
			if matches!(res, LookupResult::None) {
				seq.pop();
				None
			} else {
				Some(res)
			}
		};

		let mut primary = key;
		let mut extend_fallback: Option<Key> = None;

		if let KeyCode::Char(c) = key.code {
			if c.is_ascii_alphabetic() && key.modifiers.shift {
				let mut k = key.drop_shift();
				k.code = KeyCode::Char(c.to_ascii_uppercase());
				primary = k;

				let mut fb = key.drop_shift();
				fb.code = KeyCode::Char(c.to_ascii_lowercase());
				extend_fallback = Some(fb);
			} else if key.modifiers.shift {
				extend_fallback = Some(key.drop_shift());
			}
		} else if key.modifiers.shift {
			extend_fallback = Some(key.drop_shift());
		}

		let lookup_result = match lookup_with(primary, &mut self.key_sequence) {
			Some(res) => res,
			None => {
				if let Some(fallback) = extend_fallback {
					self.extend = true;
					lookup_with(fallback, &mut self.key_sequence).unwrap_or(LookupResult::None)
				} else {
					LookupResult::None
				}
			}
		};

		match lookup_result {
			LookupResult::Match(entry) => {
				if binding_mode != BindingMode::Normal {
					self.mode = Mode::Normal;
				}
				self.consume_action(entry.action_id)
			}
			LookupResult::Pending { sticky } => {
				if let Some(entry) = sticky {
					debug!(action = entry.action_name, keys = self.key_sequence.len(), "Pending with sticky action");
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

	/// Translates a mouse event into a [`KeyResult`].
	pub fn handle_mouse(&mut self, event: MouseEvent) -> KeyResult {
		match event {
			MouseEvent::Press {
				button: MouseButton::Left,
				row,
				col,
				..
			} => KeyResult::MouseClick { row, col, extend: self.extend },
			MouseEvent::Drag {
				button: MouseButton::Left,
				row,
				col,
				..
			} => KeyResult::MouseDrag { row, col },
			MouseEvent::Scroll { direction, .. } => KeyResult::MouseScroll { direction, count: 1 },
			_ => KeyResult::Consumed,
		}
	}
}
