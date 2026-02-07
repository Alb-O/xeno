//! Built-in hook implementations.

use std::path::Path;

use ropey::RopeSlice;
use xeno_primitives::Mode;

use crate::hook_handler;

hook_handler!(
	log_buffer_open,
	BufferOpen,
	|path: &Path, text: &RopeSlice, file_type: &Option<&str>| {
		tracing::info!(
			"Buffer opened: path={:?} type={:?} size={}",
			path,
			file_type,
			text.len_chars()
		);
	}
);

hook_handler!(
	log_mode_change,
	ModeChange,
	|old_mode: &Mode, new_mode: &Mode| {
		tracing::info!("Mode changed: {:?} -> {:?}", old_mode, new_mode);
	}
);

hook_handler!(
	log_option_change,
	OptionChanged,
	|key: &str, scope: &str| {
		tracing::info!("Option changed: key={} scope={}", key, scope);
	}
);

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	let metadata = crate::kdl::loader::load_hook_metadata();
	let handlers = inventory::iter::<crate::hooks::HookHandlerReg>
		.into_iter()
		.map(|r| r.0);
	let linked = crate::kdl::link::link_hooks(&metadata, handlers);

	for def in linked {
		builder.register_linked_hook(def);
	}
}
