use std::cell::Cell;
use std::collections::HashSet;

use xeno_primitives::{BoxFutureLocal, Key, KeyCode, Mode};
use xeno_registry::CommandError;
use xeno_registry::actions::{ActionEffects, ActionResult};
use xeno_registry::commands::{CommandContext, CommandOutcome};
use xeno_registry::hooks::{HookAction, HookContext, HookDef, HookHandler, HookMutability, HookPriority};

use super::*;
use crate::types::{InvocationOutcome, InvocationStatus, InvocationTarget};

thread_local! {
	static ACTION_PRE_COUNT: Cell<usize> = const { Cell::new(0) };
	static ACTION_POST_COUNT: Cell<usize> = const { Cell::new(0) };
	static INVOCATION_TEST_ACTION_COUNT: Cell<usize> = const { Cell::new(0) };
	static INVOCATION_TEST_ACTION_ALT_COUNT: Cell<usize> = const { Cell::new(0) };
}

fn handler_invocation_test_action(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	INVOCATION_TEST_ACTION_COUNT.with(|c| c.set(c.get() + 1));
	ActionResult::Effects(ActionEffects::ok())
}

fn handler_invocation_test_action_alt(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	INVOCATION_TEST_ACTION_ALT_COUNT.with(|c| c.set(c.get() + 1));
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_INVOCATION_TEST: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action",
		name: "invocation_test_action",
		keys: &[],
		description: "Invocation test action",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: false,
	},
	short_desc: "Invocation test action",
	handler: handler_invocation_test_action,
	bindings: &[],
};

static ACTION_INVOCATION_TEST_ALT: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action_alt",
		name: "invocation_test_action_alt",
		keys: &[],
		description: "Invocation test action alt",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: false,
	},
	short_desc: "Invocation test action alt",
	handler: handler_invocation_test_action_alt,
	bindings: &[],
};

fn handler_invocation_edit_action(_ctx: &xeno_registry::actions::ActionContext) -> ActionResult {
	ActionResult::Effects(ActionEffects::ok())
}

static ACTION_INVOCATION_EDIT: xeno_registry::actions::ActionDef = xeno_registry::actions::ActionDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_edit_action",
		name: "invocation_edit_action",
		keys: &[],
		description: "Invocation edit action",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: true,
	},
	short_desc: "Invocation edit action",
	handler: handler_invocation_edit_action,
	bindings: &[],
};

fn hook_handler_action_pre(ctx: &HookContext) -> HookAction {
	if let xeno_registry::HookEventData::ActionPre { .. } = &ctx.data {
		ACTION_PRE_COUNT.with(|count| count.set(count.get() + 1));
	}
	HookAction::done()
}

static HOOK_ACTION_PRE: HookDef = HookDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action_pre",
		name: "invocation_test_action_pre",
		keys: &[],
		description: "Count action pre hooks",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: false,
	},
	event: xeno_registry::HookEvent::ActionPre,
	mutability: HookMutability::Immutable,
	execution_priority: HookPriority::Interactive,
	handler: HookHandler::Immutable(hook_handler_action_pre),
};

fn hook_handler_action_post(ctx: &HookContext) -> HookAction {
	if let xeno_registry::HookEventData::ActionPost { .. } = &ctx.data {
		ACTION_POST_COUNT.with(|count| count.set(count.get() + 1));
	}
	HookAction::done()
}

static HOOK_ACTION_POST: HookDef = HookDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_action_post",
		name: "invocation_test_action_post",
		keys: &[],
		description: "Count action post hooks",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: false,
	},
	event: xeno_registry::HookEvent::ActionPost,
	mutability: HookMutability::Immutable,
	execution_priority: HookPriority::Interactive,
	handler: HookHandler::Immutable(hook_handler_action_post),
};

fn invocation_test_command_fail<'a>(_ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Err(CommandError::Failed("boom".into())) })
}

static CMD_TEST_FAIL: xeno_registry::commands::CommandDef = xeno_registry::commands::CommandDef {
	meta: xeno_registry::RegistryMetaStatic {
		id: "xeno-editor::invocation_test_command_fail",
		name: "invocation_test_command_fail",
		keys: &[],
		description: "Invocation test command failure",
		priority: 0,
		source: xeno_registry::RegistrySource::Crate("xeno-editor"),
		mutates_buffer: false,
	},
	handler: invocation_test_command_fail,
	user_data: None,
};

fn register_invocation_test_defs(db: &mut xeno_registry::RegistryDbBuilder) -> Result<(), xeno_registry::RegistryError> {
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_INVOCATION_TEST.clone()));
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_INVOCATION_TEST_ALT.clone()));
	db.push_domain::<xeno_registry::actions::Actions>(xeno_registry::actions::def::ActionInput::Static(ACTION_INVOCATION_EDIT.clone()));
	db.push_domain::<xeno_registry::commands::Commands>(xeno_registry::commands::def::CommandInput::Static(CMD_TEST_FAIL.clone()));
	db.push_domain::<xeno_registry::hooks::Hooks>(xeno_registry::hooks::HookInput::Static(HOOK_ACTION_PRE));
	db.push_domain::<xeno_registry::hooks::Hooks>(xeno_registry::hooks::HookInput::Static(HOOK_ACTION_POST));
	Ok(())
}

inventory::submit! {
	xeno_registry::BuiltinsReg {
		ordinal: 65000,
		f: register_invocation_test_defs,
	}
}

mod basics;
mod nu_hooks;
mod nu_macro;
mod routing;
