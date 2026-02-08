use super::spec::CommandsSpec;
use crate::commands::def::CommandHandler;
use crate::commands::entry::CommandEntry;
use crate::commands::handler::CommandHandlerStatic;
use crate::core::{
	LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol,
};

/// A command definition assembled from spec + Rust handler.
pub type LinkedCommandDef = LinkedDef<CommandPayload>;

#[derive(Clone)]
pub struct CommandPayload {
	pub handler: CommandHandler,
}

impl LinkedPayload<CommandEntry> for CommandPayload {
	fn build_entry(
		&self,
		_ctx: &mut dyn crate::core::index::BuildCtx,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> CommandEntry {
		CommandEntry {
			meta,
			handler: self.handler,
			user_data: None,
		}
	}
}

/// Links spec with handler statics, producing `LinkedCommandDef`s.
pub fn link_commands(
	spec: &CommandsSpec,
	handlers: impl Iterator<Item = &'static CommandHandlerStatic>,
) -> Vec<LinkedCommandDef> {
	crate::defs::link::link_by_name(
		&spec.commands,
		handlers,
		|m| m.common.name.as_str(),
		|h| h.name,
		|meta, handler| {
			let common = &meta.common;
			let id = format!("xeno-registry::{}", common.name);

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: common.name.clone(),
					keys: common.keys.clone(),
					description: common.description.clone(),
					priority: common.priority,
					source: RegistrySource::Crate(handler.crate_name),
					required_caps: vec![], // Commands don't currently use required_caps from KDL in link/commands.rs
					flags: common.flags,
					short_desc: Some(common.name.clone()), // commands.rs used name as short_desc
				},
				payload: CommandPayload {
					handler: handler.handler,
				},
			}
		},
		"command",
	)
}
