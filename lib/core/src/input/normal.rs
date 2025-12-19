use crate::ext::{BindingMode, find_binding};
use crate::input::InputHandler;
use crate::input::types::KeyResult;
use crate::key::Key;

impl InputHandler {
	pub(crate) fn handle_normal_key(&mut self, key: Key) -> KeyResult {
		if let Some(digit) = key.as_digit()
			&& (digit != 0 || self.count > 0)
		{
			self.count = self.count.saturating_mul(10).saturating_add(digit);
			return KeyResult::Consumed;
		}

		let key = self.extend_and_lower_if_shift(key);

		if let Some(binding) = find_binding(BindingMode::Normal, key) {
			let count = if self.count > 0 {
				self.count as usize
			} else {
				1
			};
			let extend = self.extend;
			let register = self.register;
			self.reset_params();
			return KeyResult::Action {
				name: binding.action,
				count,
				extend,
				register,
			};
		}

		self.reset_params();
		KeyResult::Unhandled
	}
}
