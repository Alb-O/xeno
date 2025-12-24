use super::HookContext;
use crate::hook;

hook!(
	log_mode_change,
	ModeChange,
	1000,
	"Log mode changes",
	|ctx| {
		if let HookContext::ModeChange { old_mode, new_mode } = ctx {
			let _ = (old_mode, new_mode);
		}
	}
);
