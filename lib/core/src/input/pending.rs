use crate::ext::{ObjectSelectionKind, PendingKind};
use crate::input::InputHandler;
use crate::input::types::{KeyResult, Mode};
use crate::key::{Key, KeyCode, SpecialKey};

impl InputHandler {
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
					KeyResult::ActionWithChar {
						name: "find_char",
						count,
						extend,
						register,
						char_arg: ch,
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
					KeyResult::ActionWithChar {
						name: "find_char_reverse",
						count,
						extend,
						register,
						char_arg: ch,
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
					KeyResult::ActionWithChar {
						name: "replace_char",
						count,
						extend,
						register,
						char_arg: ch,
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

					let kind = match selection {
						ObjectSelectionKind::Inner => Some("select_object_inner"),
						ObjectSelectionKind::Around => Some("select_object_around"),
						ObjectSelectionKind::ToStart => Some("select_object_to_start"),
						ObjectSelectionKind::ToEnd => Some("select_object_to_end"),
					};

					match kind {
						Some(action) => KeyResult::ActionWithChar {
							name: action,
							count,
							extend,
							register,
							char_arg: ch,
						},
						None => KeyResult::Consumed,
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
