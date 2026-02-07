use std::any::Any;

use super::entry::CommandEntry;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{
	CapabilitySet, FrozenInterner, RegistryMeta, RegistryMetaStatic, Symbol, SymbolList,
};

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
			aliases: StrListRef::Static(self.meta.aliases),
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
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		meta.aliases.for_each(|a| sink.push(a));
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> CommandEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		meta_ref.aliases.for_each(|alias| {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		});
		let len = (alias_pool.len() as u32 - start) as u16;

		let meta = RegistryMeta {
			id: interner.get(meta_ref.id).expect("missing interned id"),
			name: interner.get(meta_ref.name).expect("missing interned name"),
			description: interner
				.get(meta_ref.description)
				.expect("missing interned description"),
			aliases: SymbolList { start, len },
			priority: meta_ref.priority,
			source: meta_ref.source,
			required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
			flags: meta_ref.flags,
		};

		CommandEntry {
			meta,
			handler: self.handler,
			user_data: self.user_data,
		}
	}
}

/// Unified command input: either a static `CommandDef` or a KDL-linked definition.
///
/// Same pattern as `ActionInput` — lets the `RegistryBuilder` accept both
/// legacy static definitions and KDL-linked definitions through a single
/// generic `In` parameter.
pub enum CommandInput {
	/// Static definition from `command!` macro.
	Static(CommandDef),
	/// KDL-linked definition with owned metadata.
	Linked(crate::kdl::link::LinkedCommandDef),
}

impl BuildEntry<CommandEntry> for CommandInput {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match self {
			Self::Static(def) => def.meta_ref(),
			Self::Linked(def) => def.meta_ref(),
		}
	}

	fn short_desc_str(&self) -> &str {
		match self {
			Self::Static(def) => def.short_desc_str(),
			Self::Linked(def) => def.short_desc_str(),
		}
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		match self {
			Self::Static(def) => def.collect_strings(sink),
			Self::Linked(def) => def.collect_strings(sink),
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> CommandEntry {
		match self {
			Self::Static(def) => def.build(interner, alias_pool),
			Self::Linked(def) => def.build(interner, alias_pool),
		}
	}
}
