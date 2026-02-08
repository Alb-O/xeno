//! Action definition and handler types.

use std::sync::Arc;

use super::entry::ActionEntry;
use super::keybindings::KeyBindingDef;
use crate::actions::{ActionContext, ActionResult};
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{FrozenInterner, RegistryMetaStatic, Symbol};

/// Definition of a registered action (static input for builder).
///
/// Actions are the fundamental unit of editor behavior. They're registered
/// explicitly and looked up by keybindings.
#[derive(Clone)]
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

impl BuildEntry<ActionEntry> for ActionDef {
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
		self.short_desc
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		sink.push(self.short_desc);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> ActionEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		ActionEntry {
			meta,
			short_desc: interner
				.get(self.short_desc)
				.expect("missing interned short_desc"),
			handler: self.handler,
			bindings: Arc::from(self.bindings),
		}
	}
}

/// Function signature for action handlers.
///
/// Takes an immutable [`ActionContext`] and returns an [`ActionResult`]
/// describing what the editor should do.
pub type ActionHandler = fn(&ActionContext) -> ActionResult;

/// Unified action input: either a static `ActionDef` or a KDL-linked definition.
pub type ActionInput =
	crate::core::def_input::DefInput<ActionDef, crate::kdl::link::LinkedActionDef>;
