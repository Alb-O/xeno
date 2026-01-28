mod buffer;
mod edit;
mod help;
mod quit;
mod registry;
mod set;
mod theme;
mod write;

use crate::commands::CommandDef;
use crate::db::builder::{BuiltinGroup, RegistryDbBuilder};

const GROUPS: &[BuiltinGroup<CommandDef>] = &[
	BuiltinGroup::new("quit", quit::DEFS),
	BuiltinGroup::new("write", write::DEFS),
	BuiltinGroup::new("edit", edit::DEFS),
	BuiltinGroup::new("buffer", buffer::DEFS),
	BuiltinGroup::new("help", help::DEFS),
	BuiltinGroup::new("set", set::DEFS),
	BuiltinGroup::new("theme", theme::DEFS),
	BuiltinGroup::new("registry", registry::DEFS),
];

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	for group in GROUPS {
		builder.register_command_group(group);
	}
}
