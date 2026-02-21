//! Input handler managing key processing and mode state.

use tracing::debug;
use xeno_keymap_core::parser::Node;
use xeno_primitives::{Key, KeyCode, MouseButton, MouseEvent};
use xeno_registry::actions::BindingMode;
use xeno_registry::keymaps::KeymapBehavior;
use xeno_registry::{KeymapSnapshot, LookupOutcome, get_keymap_snapshot};

use super::keymap_adapter::key_to_node;
use super::types::{KeyDispatch, KeyResult, Mode};

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

	/// Consumes state and produces the appropriate [`KeyResult`] for a binding entry.
	pub(crate) fn consume_binding(&mut self, entry: &xeno_registry::CompiledBinding) -> KeyResult {
		match entry.target() {
			xeno_registry::CompiledBindingTarget::Action { count, extend, register, .. } => {
				// Multiply prefix count with binding count; OR extends; prefix register wins.
				// Clamp to MAX_ACTION_COUNT to prevent overflow and DoS via huge counts.
				let max = xeno_registry::MAX_ACTION_COUNT;
				let prefix_count = (self.count as usize).max(1).min(max);
				let binding_count = (*count).max(1);
				let final_count = prefix_count.saturating_mul(binding_count).min(max);
				let final_extend = self.extend || *extend;
				let final_register = self.register.or(*register);
				self.reset_params();
				KeyResult::Dispatch(KeyDispatch {
					invocation: xeno_registry::Invocation::Action {
						name: entry.name().to_string(),
						count: final_count,
						extend: final_extend,
						register: final_register,
					},
				})
			}
			xeno_registry::CompiledBindingTarget::Invocation { inv } => {
				let inv = inv.clone();
				self.reset_params();
				KeyResult::Dispatch(KeyDispatch { invocation: inv })
			}
		}
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

	/// Dispatches a key through the current mode's handler with default behavior.
	pub fn handle_key(&mut self, key: Key) -> KeyResult {
		let registry = get_keymap_snapshot();
		self.handle_key_with_registry(key, &registry, KeymapBehavior::default())
	}

	/// Dispatches a key using an explicit keymap registry and behavior flags.
	pub fn handle_key_with_registry(&mut self, key: Key, registry: &KeymapSnapshot, behavior: KeymapBehavior) -> KeyResult {
		match &self.mode {
			Mode::Normal => self.handle_mode_key(key, BindingMode::Normal, registry, behavior),
			Mode::Insert => self.handle_insert_key(key, registry),
			Mode::PendingAction(kind) => {
				let kind = *kind;
				self.handle_pending_action_key(key, kind)
			}
		}
	}

	/// Resolves a key against a binding mode's keymap.
	///
	/// Behavior-flag-dependent logic:
	/// * `vim_shift_letter_casefold`: Shift+alphabetic canonicalizes to uppercase
	///   for lookup (Vim: Shift+n → "N"). Fallback tries lowercase with `extend = true`.
	///   When disabled, Shift is kept as a modifier (emacs semantics).
	/// * `normal_digit_prefix_count`: Bare digits accumulate a count prefix in Normal mode.
	fn handle_mode_key(&mut self, key: Key, binding_mode: BindingMode, registry: &KeymapSnapshot, behavior: KeymapBehavior) -> KeyResult {
		if behavior.normal_digit_prefix_count
			&& binding_mode == BindingMode::Normal
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

		let lookup_with = |k: Key, seq: &mut Vec<Node>| -> Option<LookupOutcome> {
			let node = key_to_node(k);
			seq.push(node);
			let res = registry.lookup(binding_mode, seq);
			if matches!(res, LookupOutcome::None) {
				seq.pop();
				None
			} else {
				Some(res)
			}
		};

		let mut primary = key;
		let mut extend_fallback: Option<Key> = None;

		if behavior.vim_shift_letter_casefold {
			// Vim semantics: Shift+n → lookup "N", fallback to "n" with extend.
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
		} else {
			// Non-casefold: keep Shift as-is for char keys. For non-char keys,
			// shift-arrow selection extend fallback remains.
			if !matches!(key.code, KeyCode::Char(_)) && key.modifiers.shift {
				extend_fallback = Some(key.drop_shift());
			}
		}

		let lookup_result = match lookup_with(primary, &mut self.key_sequence) {
			Some(res) => res,
			None => {
				if let Some(fallback) = extend_fallback {
					self.extend = true;
					lookup_with(fallback, &mut self.key_sequence).unwrap_or(LookupOutcome::None)
				} else {
					LookupOutcome::None
				}
			}
		};

		match lookup_result {
			LookupOutcome::Match(entry) => {
				if binding_mode != BindingMode::Normal {
					self.mode = Mode::Normal;
				}
				self.consume_binding(entry)
			}
			LookupOutcome::Pending { sticky } => {
				if let Some(entry) = sticky {
					debug!(action = entry.name(), keys = self.key_sequence.len(), "Pending with sticky action");
				}
				KeyResult::Pending {
					keys_so_far: self.key_sequence.len(),
				}
			}
			LookupOutcome::None => {
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
