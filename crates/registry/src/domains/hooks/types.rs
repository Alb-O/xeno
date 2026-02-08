//! Hook type definitions: HookDef, HookAction, HookResult.

use super::context::{HookContext, MutableHookContext};
use crate::HookEvent;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	FrozenInterner, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistryMetadata, Symbol,
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

impl BuildEntry<HookEntry> for HookDef {
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

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> HookEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		HookEntry {
			meta,
			event: self.event,
			mutability: self.mutability,
			execution_priority: self.execution_priority,
			handler: self.handler,
		}
	}
}

/// Unified input for hook registration.
pub type HookInput = crate::core::def_input::DefInput<HookDef, crate::hooks::link::LinkedHookDef>;
