//! Hook type definitions: HookDef, HookAction, HookResult.

use super::context::{HookContext, MutableHookContext};
use crate::HookEvent;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	CapabilitySet, FrozenInterner, RegistryEntry, RegistryMeta, RegistryMetaStatic,
	RegistryMetadata, Symbol, SymbolList,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HookPriority {
	#[default]
	Interactive,
	Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HookResult {
	#[default]
	Continue,
	Cancel,
}

pub type HookFuture = xeno_primitives::BoxFutureStatic<HookResult>;

pub enum HookAction {
	Done(HookResult),
	Async(HookFuture),
}

impl HookAction {
	pub fn done() -> Self {
		HookAction::Done(HookResult::Continue)
	}

	pub fn cancel() -> Self {
		HookAction::Done(HookResult::Cancel)
	}
}

impl From<HookResult> for HookAction {
	fn from(result: HookResult) -> Self {
		HookAction::Done(result)
	}
}

impl From<()> for HookAction {
	fn from(_: ()) -> Self {
		HookAction::Done(HookResult::Continue)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMutability {
	Immutable,
	Mutable,
}

#[derive(Clone, Copy)]
pub enum HookHandler {
	Immutable(fn(&HookContext) -> HookAction),
	Mutable(fn(&mut MutableHookContext) -> HookAction),
}

/// A hook that responds to editor events (static input).
#[derive(Clone, Copy)]
pub struct HookDef {
	pub meta: RegistryMetaStatic,
	pub event: HookEvent,
	pub mutability: HookMutability,
	pub execution_priority: HookPriority,
	pub handler: HookHandler,
}

impl std::fmt::Debug for HookDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HookDef")
			.field("name", &self.meta.name)
			.field("event", &self.event)
			.field("mutability", &self.mutability)
			.field("execution_priority", &self.execution_priority)
			.finish()
	}
}

/// Symbolized hook entry.
pub struct HookEntry {
	pub meta: RegistryMeta,
	pub event: HookEvent,
	pub mutability: HookMutability,
	pub execution_priority: HookPriority,
	pub handler: HookHandler,
}

crate::impl_registry_entry!(HookEntry);

/// Unified input for hook registration.
pub enum HookInput {
	Static(HookDef),
	Linked(crate::kdl::link::LinkedHookDef),
}

impl BuildEntry<HookEntry> for HookInput {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match self {
			Self::Static(d) => d.meta_ref(),
			Self::Linked(d) => d.meta_ref(),
		}
	}

	fn short_desc_str(&self) -> &str {
		match self {
			Self::Static(d) => d.short_desc_str(),
			Self::Linked(d) => d.short_desc_str(),
		}
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		match self {
			Self::Static(d) => d.collect_strings(sink),
			Self::Linked(d) => d.collect_strings(sink),
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> HookEntry {
		match self {
			Self::Static(d) => d.build(interner, alias_pool),
			Self::Linked(d) => d.build(interner, alias_pool),
		}
	}
}

impl BuildEntry<HookEntry> for HookDef {
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

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> HookEntry {
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

		HookEntry {
			meta,
			event: self.event,
			mutability: self.mutability,
			execution_priority: self.execution_priority,
			handler: self.handler,
		}
	}
}
