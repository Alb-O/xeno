use xeno_registry::{
	ActionResult, EditorContext, HookContext, HookEventData, dispatch_result,
	emit_sync_with as emit_hook_sync_with,
};

use crate::impls::Editor;

impl Editor {
	/// Executes a named action with the given count and options.
	#[allow(dead_code, reason = "Legacy wrapper retained for migration.")]
	#[deprecated(note = "Use Editor::invoke_action instead.")]
	pub(crate) fn execute_action(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
	) -> bool {
		self.invoke_action(name, count, extend, register, None)
			.is_quit()
	}

	/// Executes an action that requires an additional character argument.
	#[allow(dead_code, reason = "Legacy wrapper retained for migration.")]
	#[deprecated(note = "Use Editor::invoke_action instead.")]
	pub(crate) fn execute_action_with_char(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: char,
	) -> bool {
		self.invoke_action(name, count, extend, register, Some(char_arg))
			.is_quit()
	}

	/// Dispatches an action result to handlers and emits post-action hook.
	pub(crate) fn apply_action_result(
		&mut self,
		action_id: &'static str,
		result: ActionResult,
		extend: bool,
	) -> bool {
		let mut ctx = EditorContext::new(self);
		let result_variant = result.variant_name();
		let should_quit = dispatch_result(&result, &mut ctx, extend);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::ActionPost {
					action_id,
					result_variant,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
		should_quit
	}
}
