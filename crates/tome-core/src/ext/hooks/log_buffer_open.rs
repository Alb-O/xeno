use linkme::distributed_slice;

use super::{HOOKS, HookContext, HookDef, HookEvent};

#[distributed_slice(HOOKS)]
static HOOK_LOG_BUFFER_OPEN: HookDef = HookDef {
	name: "log_buffer_open",
	event: HookEvent::BufferOpen,
	description: "Log when a buffer is opened",
	priority: 1000,
	handler: |ctx| {
		if let HookContext::BufferOpen {
			path, file_type, ..
		} = ctx
		{
			let ft = file_type.unwrap_or("unknown");
			// In a real implementation, this would use a proper logging system
			let _ = (path, ft);
		}
	},
};
