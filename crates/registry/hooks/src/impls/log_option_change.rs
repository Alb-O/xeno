//! Example hook demonstrating OptionChanged event handling.

use crate::{HookEventData, hook};

hook!(
	log_option_change,
	OptionChanged,
	1000,
	"Log option changes for debugging",
	|ctx| {
		if let HookEventData::OptionChanged { key, scope } = &ctx.data {
			tracing::debug!(option = *key, scope = *scope, "option changed");
		}
	}
);
