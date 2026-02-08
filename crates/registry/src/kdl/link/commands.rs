use super::*;
use crate::commands::def::CommandHandler;
use crate::commands::entry::CommandEntry;
use crate::commands::handler::CommandHandlerStatic;
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::CommandsBlob;

/// A command definition assembled from KDL metadata + Rust handler.
pub type LinkedCommandDef = LinkedDef<CommandPayload>;

#[derive(Clone)]
pub struct CommandPayload {
	pub handler: CommandHandler,
}

impl LinkedPayload<CommandEntry> for CommandPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
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

/// Links KDL command metadata with handler statics, producing `LinkedCommandDef`s.
///
/// Panics if any KDL command has no matching handler, or vice versa.
pub fn link_commands(
	metadata: &CommandsBlob,
	handlers: impl Iterator<Item = &'static CommandHandlerStatic>,
) -> Vec<LinkedCommandDef> {
	super::common::link_by_name(
		&metadata.commands,
		handlers,
		|m| &m.name,
		|h| h.name,
		|meta, handler| LinkedDef {
			meta: LinkedMetaOwned {
				id: format!("xeno-registry::{}", meta.name),
				name: meta.name.clone(),
				keys: meta.keys.clone(),
				description: meta.description.clone(),
				priority: 0,
				source: RegistrySource::Crate(handler.crate_name),
				required_caps: vec![],
				flags: 0,
				short_desc: None,
			},
			payload: CommandPayload {
				handler: handler.handler,
			},
		},
		"command",
	)
}
