use crate::input::InputHandler;
use crate::input::types::{KeyResult, Mode};
use crate::key::{Key, KeyCode, SpecialKey};

impl InputHandler {
	pub(crate) fn handle_command_key(
		&mut self,
		key: Key,
		prompt: char,
		mut input: String,
	) -> KeyResult {
		let key = if key.modifiers.shift
			&& let KeyCode::Char(c) = key.code
			&& c.is_ascii_lowercase()
		{
			key.normalize()
		} else {
			key
		};

		match key.code {
			KeyCode::Special(SpecialKey::Escape) => {
				self.mode = Mode::Normal;
				self.reset_params();
				KeyResult::ModeChange(Mode::Normal)
			}

			KeyCode::Special(SpecialKey::Enter) => {
				self.mode = Mode::Normal;
				self.reset_params();

				if input.is_empty() {
					return KeyResult::Consumed;
				}

				match prompt {
					':' => KeyResult::ExecuteCommand(input),
					'/' => KeyResult::ExecuteSearch {
						pattern: input,
						reverse: false,
					},
					'?' => KeyResult::ExecuteSearch {
						pattern: input,
						reverse: true,
					},
					's' => KeyResult::SelectRegex { pattern: input },
					'S' => KeyResult::SplitRegex { pattern: input },
					'k' => KeyResult::KeepMatching { pattern: input },
					'K' => KeyResult::KeepNotMatching { pattern: input },
					'|' => KeyResult::PipeReplace { command: input },
					'\\' => KeyResult::PipeIgnore { command: input },
					'!' => KeyResult::InsertOutput { command: input },
					'@' => KeyResult::AppendOutput { command: input },
					_ => KeyResult::Consumed,
				}
			}

			KeyCode::Special(SpecialKey::Backspace) => {
				if input.is_empty() {
					self.mode = Mode::Normal;
					self.reset_params();
					KeyResult::ModeChange(Mode::Normal)
				} else {
					input.pop();
					self.mode = Mode::Command { prompt, input };
					KeyResult::Consumed
				}
			}

			KeyCode::Char(c) if key.modifiers.is_empty() => {
				input.push(c);
				self.mode = Mode::Command { prompt, input };
				KeyResult::Consumed
			}

			KeyCode::Char(' ') => {
				input.push(' ');
				self.mode = Mode::Command { prompt, input };
				KeyResult::Consumed
			}

			_ => KeyResult::Consumed,
		}
	}
}
