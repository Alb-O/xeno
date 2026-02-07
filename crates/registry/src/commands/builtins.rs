mod buffer;
mod edit;
mod help;
mod quit;
mod registry;
mod set;
mod theme;
mod write;

use crate::db::builder::RegistryDbBuilder;
use crate::kdl::link::link_commands;
use crate::kdl::loader::load_command_metadata;

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	let metadata = load_command_metadata();
	let handlers = inventory::iter::<crate::commands::CommandHandlerReg>
		.into_iter()
		.map(|r| r.0);
	let linked = link_commands(&metadata, handlers);
	for def in linked {
		builder.register_linked_command(def);
	}
}
