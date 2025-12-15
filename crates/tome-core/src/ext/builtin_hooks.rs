//! Built-in hook registrations.
//!
//! These are default hooks that provide basic editor functionality.
//! They can be overridden or supplemented by user-defined hooks.

use linkme::distributed_slice;

use super::hooks::{HookContext, HookDef, HookEvent, HOOKS};

#[distributed_slice(HOOKS)]
static HOOK_LOG_BUFFER_OPEN: HookDef = HookDef {
    name: "log_buffer_open",
    event: HookEvent::BufferOpen,
    description: "Log when a buffer is opened",
    priority: 1000,
    handler: |ctx| {
        if let HookContext::BufferOpen { path, file_type, .. } = ctx {
            let ft = file_type.unwrap_or("unknown");
            // In a real implementation, this would use a proper logging system
            // For now, this is a no-op placeholder that demonstrates the pattern
            let _ = (path, ft);
        }
    },
};

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
