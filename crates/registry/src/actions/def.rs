//! Action definition and handler types.

use super::entry::ActionEntry;
use super::keybindings::KeyBindingDef;
use crate::actions::{ActionContext, ActionResult};
use crate::core::index::{BuildEntry, RegistryMetaRef};
use crate::core::{
	CapabilitySet, FrozenInterner, RegistryMeta, RegistryMetaStatic, Symbol, SymbolList,
};

/// Definition of a registered action (static input for builder).
///
/// Actions are the fundamental unit of editor behavior. They're registered
/// explicitly and looked up by keybindings.
#[derive(Copy, Clone)]
pub struct ActionDef {
	/// Common registry metadata (static).
	pub meta: RegistryMetaStatic,
	/// Short description without key-sequence prefix (for which-key HUD).
	pub short_desc: &'static str,
	/// The function that executes this action.
	pub handler: ActionHandler,
	/// Keybindings associated with the action.
	pub bindings: &'static [KeyBindingDef],
}

impl crate::core::RegistryEntry for ActionDef {
	fn meta(&self) -> &crate::core::RegistryMeta {
		// This is a dummy implementation to satisfy trait bounds in Key.
		// It should never be called on a static ActionDef via the RegistryEntry trait.
		panic!("Called meta() on static ActionDef")
	}
}

impl BuildEntry<ActionEntry> for ActionDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			aliases: self.meta.aliases,
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.short_desc
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		for &alias in meta.aliases {
			sink.push(alias);
		}
		sink.push(self.short_desc);
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> ActionEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;

		// Dedup aliases per entry
		let mut unique_aliases = Vec::from(meta_ref.aliases);
		unique_aliases.sort_unstable();
		unique_aliases.dedup();

		for alias in unique_aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		let len = (alias_pool.len() as u32 - start) as u16;
		debug_assert!(alias_pool.len() as u32 - start <= u16::MAX as u32);

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

		ActionEntry {
			meta,
			short_desc: interner
				.get(self.short_desc)
				.expect("missing interned short_desc"),
			handler: self.handler,
			bindings: self.bindings,
		}
	}
}

/// Function signature for action handlers.
///
/// Takes an immutable [`ActionContext`] and returns an [`ActionResult`]
/// describing what the editor should do.
pub type ActionHandler = fn(&ActionContext) -> ActionResult;
