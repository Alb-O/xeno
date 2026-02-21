use std::any::Any;

use super::entry::CommandEntry;
use super::spec::CommandPaletteSpec;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{RegistryMetaStatic, Symbol};

/// Function signature for async command handlers.
pub type CommandHandler =
	for<'a> fn(&'a mut super::CommandContext<'a>) -> xeno_primitives::BoxFutureLocal<'a, Result<super::CommandOutcome, crate::core::CommandError>>;

/// A registered command definition (static input for builder).
#[derive(Clone)]
pub struct CommandDef {
	/// Common registry metadata (static).
	pub meta: RegistryMetaStatic,
	/// Async function that executes the command.
	pub handler: CommandHandler,
	/// Extension-specific data attached to the command.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

impl BuildEntry<CommandEntry> for CommandDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			mutates_buffer: self.meta.mutates_buffer,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_payload_strings<'b>(&'b self, _collector: &mut crate::core::index::StringCollector<'_, 'b>) {}

	fn build(&self, ctx: &mut dyn crate::core::index::BuildCtx, key_pool: &mut Vec<Symbol>) -> CommandEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		CommandEntry {
			meta,
			palette: CommandPaletteSpec::default(),
			handler: self.handler,
			user_data: self.user_data,
		}
	}
}

/// Unified command input: either a static `CommandDef` or a registry-linked definition.
pub type CommandInput = crate::core::def_input::DefInput<CommandDef, crate::commands::link::LinkedCommandDef>;
