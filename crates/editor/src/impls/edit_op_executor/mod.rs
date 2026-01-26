//! Data-oriented edit operation executor.

mod ops;
mod types;

use xeno_primitives::EditOrigin;
use xeno_registry::edit_op::{EditOp, EditPlan};

use super::Editor;

impl Editor {
	/// Executes a data-oriented edit operation.
	///
	/// Compiles the operation into an [`EditPlan`] with resolved policies,
	/// then executes it using the compile -> commit pattern.
	pub fn execute_edit_op(&mut self, op: EditOp) {
		let plan = op.compile();
		self.execute_edit_plan(plan);
	}

	/// Executes a compiled edit plan using the compile -> commit pattern.
	pub fn execute_edit_plan(&mut self, plan: EditPlan) {
		if plan.op.modifies_text() && !self.guard_readonly() {
			return;
		}

		for pre in &plan.op.pre {
			self.apply_pre_effect(pre);
		}

		if !self.apply_selection_op(&plan.op.selection) {
			return;
		}

		let original_cursor = self.buffer().cursor;

		if let Some((tx, new_selection)) = self.build_transform_transaction(&plan) {
			let buffer_id = self.focused_view();
			self.apply_edit(
				buffer_id,
				&tx,
				Some(new_selection),
				plan.undo_policy,
				EditOrigin::Internal("edit_op"),
			);
		}

		for post in &plan.op.post {
			self.apply_post_effect(post, original_cursor);
		}
	}
}
