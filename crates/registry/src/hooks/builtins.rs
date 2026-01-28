//! Built-in hook implementations.

use std::path::Path;

use ropey::RopeSlice;
use xeno_primitives::Mode;

use crate::hook;

hook!(
	log_buffer_open,
	BufferOpen,
	1000,
	"Log buffer open",
	|path: &Path, text: &RopeSlice, file_type: &Option<&str>| {
		tracing::info!(
			"Buffer opened: path={:?} type={:?} size={}",
			path,
			file_type,
			text.len_chars()
		);
	}
);

hook!(
	log_mode_change,
	ModeChange,
	1000,
	"Log mode change",
	|old_mode: &Mode, new_mode: &Mode| {
		tracing::info!("Mode changed: {:?} -> {:?}", old_mode, new_mode);
	}
);

hook!(
	log_option_change,
	OptionChanged,
	1000,
	"Log option change",
	|key: &str, scope: &str| {
		tracing::info!("Option changed: key={} scope={}", key, scope);
	}
);

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_hook(&HOOK_log_buffer_open);
	builder.register_hook(&HOOK_log_mode_change);
	builder.register_hook(&HOOK_log_option_change);
}
