use crate::InputHandler;
use crate::types::KeyResult;
use tome_base::key::Key;
use tome_manifest::{BindingMode, find_binding_resolved};

impl InputHandler {
	pub(crate) fn handle_normal_key(&mut self, key: Key) -> KeyResult {
		if let Some(digit) = key.as_digit()
			&& (digit != 0 || self.count > 0)
		{
			self.count = self.count.saturating_mul(10).saturating_add(digit);
			return KeyResult::Consumed;
		}

		let key = self.extend_and_lower_if_shift(key);

		// Use typed ActionId dispatch
		if let Some(resolved) = find_binding_resolved(BindingMode::Normal, key) {
			let count = if self.count > 0 {
				self.count as usize
			} else {
				1
			};
			let extend = self.extend;
			let register = self.register;
			self.reset_params();
			return KeyResult::ActionById {
				id: resolved.action_id,
				count,
				extend,
				register,
			};
		}

		self.reset_params();
		KeyResult::Unhandled
	}
}
