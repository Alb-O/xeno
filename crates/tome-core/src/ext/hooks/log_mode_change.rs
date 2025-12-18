use linkme::distributed_slice;

use super::{HOOKS, HookContext, HookDef, HookEvent};

#[distributed_slice(HOOKS)]
static HOOK_LOG_MODE_CHANGE: HookDef = HookDef {
	name: "log_mode_change",
	event: HookEvent::ModeChange,
	description: "Log mode changes",
	priority: 1000,
	handler: |ctx| {
		if let HookContext::ModeChange { old_mode, new_mode } = ctx {
			let _ = (old_mode, new_mode);
		}
	},
};
