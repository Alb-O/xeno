use super::*;
use crate::commands::def::CommandHandler;
use crate::commands::entry::CommandEntry;
use crate::commands::handler::CommandHandlerStatic;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::{CommandMetaRaw, CommandsBlob};

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
	super::spec::link_domain::<CommandLinkSpec>(&metadata.commands, handlers)
}

struct CommandLinkSpec;

impl super::spec::DomainLinkSpec for CommandLinkSpec {
	type Meta = CommandMetaRaw;
	type HandlerFn = CommandHandler;
	type Entry = CommandEntry;
	type Payload = CommandPayload;

	const WHAT: &'static str = "command";
	const CANONICAL_PREFIX: &'static str = "xeno-registry::";

	fn common(meta: &Self::Meta) -> &crate::kdl::types::MetaCommonRaw {
		&meta.common
	}

	fn short_desc(meta: &Self::Meta) -> String {
		meta.common.name.clone()
	}

	fn build_payload(
		_meta: &Self::Meta,
		handler: Self::HandlerFn,
		_canonical_id: std::sync::Arc<str>,
	) -> Self::Payload {
		CommandPayload { handler }
	}
}
