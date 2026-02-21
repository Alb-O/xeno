use xeno_primitives::{Key, KeyCode, ObjectSelectionKind, PendingKind};

use super::InputHandler;
use super::types::{KeyDispatch, KeyResult, Mode};

impl InputHandler {
	/// Handles key input for pending actions (character find, text objects, etc.).
	pub(crate) fn handle_pending_action_key(&mut self, key: Key, pending: PendingKind) -> KeyResult {
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		let action_name = match pending {
			PendingKind::FindChar { .. } => "find_char",
			PendingKind::FindCharReverse { .. } => "find_char_reverse",
			PendingKind::ReplaceChar => "replace_char",
			PendingKind::Object(selection) => match selection {
				ObjectSelectionKind::Inner => "select_object_inner",
				ObjectSelectionKind::Around => "select_object_around",
				ObjectSelectionKind::ToStart => "select_object_to_start",
				ObjectSelectionKind::ToEnd => "select_object_to_end",
			},
		};

		match key.code {
			KeyCode::Char(ch) => {
				let count = self.effective_count() as usize;
				let extend = self.extend;
				let register = self.register;
				self.reset_params();
				KeyResult::Dispatch(KeyDispatch {
					invocation: xeno_registry::Invocation::ActionWithChar {
						name: action_name.to_string(),
						count,
						extend,
						register,
						char_arg: ch,
					},
				})
			}
			KeyCode::Esc => {
				self.mode = Mode::Normal;
				self.reset_params();
				KeyResult::ModeChange(Mode::Normal)
			}
			_ => KeyResult::Consumed,
		}
	}
}
