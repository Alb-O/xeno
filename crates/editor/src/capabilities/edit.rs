use xeno_registry::actions::{EditAccess, edit_op};

use crate::capabilities::provider::EditorCaps;

impl EditAccess for EditorCaps<'_> {
	fn execute_edit_op(&mut self, op: &edit_op::EditOp) {
		self.ed.execute_edit_op(op.clone());
	}

	fn paste(&mut self, before: bool) {
		if before {
			self.ed.paste_before();
		} else {
			self.ed.paste_after();
		}
	}
}
