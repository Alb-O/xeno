use tracing::{trace, trace_span};
use xeno_registry::actions::editor_ctx::HandleOutcome;
use xeno_registry::actions::{ActionArgs, ActionContext, ActionResult, find_action};
use xeno_registry::hooks::{HookContext, emit_sync_with as emit_hook_sync_with};
use xeno_registry::{HookEventData, RegistryEntry};

use crate::editor_ctx::apply_effects;
use crate::impls::Editor;
use crate::impls::invocation::kernel::InvocationKernel;
use crate::impls::invocation::policy_gate::InvocationGateInput;
use crate::types::{InvocationOutcome, InvocationPolicy, InvocationTarget};

impl Editor {
	pub(crate) fn run_action_invocation(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: Option<char>,
		policy: InvocationPolicy,
	) -> InvocationOutcome {
		let mut kernel = InvocationKernel::new(self, policy);
		let Some(action) = find_action(name) else {
			kernel.editor().show_notification(xeno_registry::notifications::keys::unknown_action(name));
			return InvocationOutcome::not_found(InvocationTarget::Action, format!("action:{name}"));
		};

		let gate_input = InvocationGateInput::action(name, action.required_caps());
		if let Some(result) = kernel.deny_if_policy_blocks(gate_input) {
			return result;
		}

		let action_id_str = action.id_str().to_string();
		let action_name_str = action.name_str().to_string();

		emit_hook_sync_with(
			&HookContext::new(HookEventData::ActionPre { action_id: &action_id_str }),
			&mut kernel.editor().state.work_scheduler,
		);

		let span = trace_span!(
			"action",
			name = %action_name_str,
			id = %action_id_str,
			count = count,
			extend = extend,
		);
		let _guard = span.enter();

		kernel.editor().buffer_mut().ensure_valid_selection();
		let (content, cursor, selection) = {
			let buffer = kernel.editor().buffer();
			(buffer.with_doc(|doc| doc.content().clone()), buffer.cursor, buffer.selection.clone())
		};

		let ctx = ActionContext {
			text: content.slice(..),
			cursor,
			selection: &selection,
			count,
			extend,
			register,
			args: ActionArgs { char: char_arg, string: None },
		};

		let result = (action.handler)(&ctx);
		trace!(result = ?result, "Action completed");

		let outcome = if kernel.editor().apply_action_result(&action_id_str, result, extend) {
			InvocationOutcome::quit(InvocationTarget::Action)
		} else {
			InvocationOutcome::ok(InvocationTarget::Action)
		};

		kernel.flush_effects_and_return(outcome)
	}

	/// Dispatches an action result to handlers and emits post-action hook.
	pub(crate) fn apply_action_result(&mut self, action_id: &str, result: ActionResult, extend: bool) -> bool {
		let (should_quit, result_variant) = {
			let mut caps = self.caps();
			let mut ctx = xeno_registry::actions::EditorContext::new(&mut caps);
			let result_variant = result.variant_name();
			let ActionResult::Effects(effects) = result;
			let should_quit = matches!(apply_effects(&effects, &mut ctx, extend), HandleOutcome::Quit);
			(should_quit, result_variant)
		};

		emit_hook_sync_with(
			&HookContext::new(HookEventData::ActionPost { action_id, result_variant }),
			&mut self.state.work_scheduler,
		);
		should_quit
	}
}
