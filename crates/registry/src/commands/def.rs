use std::any::Any;

use super::entry::CommandEntry;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{FrozenInterner, RegistryMetaStatic, Symbol};

/// Function signature for async command handlers.
pub type CommandHandler = for<'a> fn(
	&'a mut super::CommandContext<'a>,
) -> xeno_primitives::BoxFutureLocal<
	'a,
	Result<super::CommandOutcome, crate::core::CommandError>,
>;

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
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> CommandEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		CommandEntry {
			meta,
			handler: self.handler,
			user_data: self.user_data,
		}
	}
}

/// Unified command input: either a static `CommandDef` or a KDL-linked definition.
pub type CommandInput =
	crate::core::def_input::DefInput<CommandDef, crate::commands::link::LinkedCommandDef>;
