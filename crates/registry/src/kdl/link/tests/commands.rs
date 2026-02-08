use std::collections::HashSet;

use crate::kdl::loader::load_command_metadata;

#[test]
fn all_kdl_commands_have_handlers() {
	use crate::commands::handler::CommandHandlerStatic;
	let blob = load_command_metadata();
	let handlers: Vec<&CommandHandlerStatic> =
		inventory::iter::<crate::commands::CommandHandlerReg>
			.into_iter()
			.map(|r| r.0)
			.collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

	for cmd in &blob.commands {
		assert!(
			handler_names.contains(cmd.common.name.as_str()),
			"KDL command '{}' has no handler",
			cmd.common.name
		);
	}
}

#[test]
fn all_command_handlers_have_kdl_entries() {
	let blob = load_command_metadata();
	let kdl_names: HashSet<&str> = blob
		.commands
		.iter()
		.map(|c| c.common.name.as_str())
		.collect();

	for reg in inventory::iter::<crate::commands::CommandHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"command_handler!({}) has no KDL entry",
			reg.0.name
		);
	}
}
