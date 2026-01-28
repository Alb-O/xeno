//! Built-in command implementations.

mod buffer;
mod edit;
mod help;
mod quit;
mod registry;
mod set;
mod theme;
mod write;

use crate::commands::CommandDef;
use crate::db::builder::RegistryDbBuilder;

fn register_slice(builder: &mut RegistryDbBuilder, defs: &[&'static CommandDef]) {
	for def in defs {
		builder.register_command(def);
	}
}

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	register_slice(builder, quit::DEFS);
	register_slice(builder, write::DEFS);
	register_slice(builder, edit::DEFS);
	register_slice(builder, buffer::DEFS);
	register_slice(builder, help::DEFS);
	register_slice(builder, set::DEFS);
	register_slice(builder, theme::DEFS);
	register_slice(builder, registry::DEFS);
}
