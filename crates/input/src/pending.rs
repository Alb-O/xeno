use tome_base::key::{Key, KeyCode, SpecialKey};
use tome_manifest::{ObjectSelectionKind, PendingKind, resolve_action_id};

use crate::InputHandler;
use crate::types::{KeyResult, Mode};

impl InputHandler {
	/// Handles key input for pending actions (character find, text objects, etc.).
	///
	/// Uses typed action dispatch via [`resolve_action_id`] when available, falling back
	/// to name-based dispatch for dynamic resolution.
	pub(crate) fn handle_pending_action_key(
		&mut self,
		key: Key,
		pending: PendingKind,
	) -> KeyResult {
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		match pending {
			PendingKind::FindChar { inclusive: _ } => match key.code {
				KeyCode::Char(ch) => {
				let count = self.effective_count() as usize;
				let extend = self.extend;
				let register = self.register;
				self.reset_params();
				if let Some(id) = resolve_action_id("find_char") {
						KeyResult::ActionByIdWithChar {
							id,
							count,
							extend,
							register,
							char_arg: ch,
						}
					} else {
						KeyResult::ActionWithChar {
							name: "find_char",
							count,
							extend,
							register,
							char_arg: ch,
						}
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},

			PendingKind::FindCharReverse { inclusive: _ } => match key.code {
				KeyCode::Char(ch) => {
				let count = self.effective_count() as usize;
				let extend = self.extend;
				let register = self.register;
				self.reset_params();
				if let Some(id) = resolve_action_id("find_char_reverse") {
						KeyResult::ActionByIdWithChar {
							id,
							count,
							extend,
							register,
							char_arg: ch,
						}
					} else {
						KeyResult::ActionWithChar {
							name: "find_char_reverse",
							count,
							extend,
							register,
							char_arg: ch,
						}
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},

			PendingKind::ReplaceChar => match key.code {
				KeyCode::Char(ch) => {
				let count = self.effective_count() as usize;
				let extend = self.extend;
				let register = self.register;
				self.reset_params();
				if let Some(id) = resolve_action_id("replace_char") {
						KeyResult::ActionByIdWithChar {
							id,
							count,
							extend,
							register,
							char_arg: ch,
						}
					} else {
						KeyResult::ActionWithChar {
							name: "replace_char",
							count,
							extend,
							register,
							char_arg: ch,
						}
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},

			PendingKind::Object(selection) => match key.code {
				KeyCode::Char(ch) => {
					let count = self.effective_count() as usize;
					let extend = self.extend;
					let register = self.register;
					self.reset_params();

				let action_name = match selection {
					ObjectSelectionKind::Inner => "select_object_inner",
					ObjectSelectionKind::Around => "select_object_around",
					ObjectSelectionKind::ToStart => "select_object_to_start",
					ObjectSelectionKind::ToEnd => "select_object_to_end",
				};

				if let Some(id) = resolve_action_id(action_name) {
						KeyResult::ActionByIdWithChar {
							id,
							count,
							extend,
							register,
							char_arg: ch,
						}
					} else {
						KeyResult::ActionWithChar {
							name: action_name,
							count,
							extend,
							register,
							char_arg: ch,
						}
					}
				}
				KeyCode::Special(SpecialKey::Escape) => {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				}
				_ => KeyResult::Consumed,
			},
		}
	}
}
