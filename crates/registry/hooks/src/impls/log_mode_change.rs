use crate::{HookEventData, hook};

hook!(
	log_mode_change,
	ModeChange,
	1000,
	"Log mode changes",
	|ctx| {
		if let HookEventData::ModeChange { old_mode, new_mode } = &ctx.data {
			let _ = (old_mode, new_mode);
		}
	}
);
