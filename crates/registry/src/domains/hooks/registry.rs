use crate::HookEvent;
use crate::core::index::RegistryRef;
use crate::core::{DenseId, HookId, RegistryIndex, RuntimeRegistry};
use crate::hooks::{HookDef, HookEntry};

/// Guard object that keeps a hooks snapshot alive while providing access to a definition.
pub type HooksRef = RegistryRef<HookEntry, HookId>;

pub struct HooksRegistry {
	pub(super) inner: RuntimeRegistry<HookEntry, HookId>,
}

impl HooksRegistry {
	pub fn new(builtins: RegistryIndex<HookEntry, HookId>) -> Self {
		Self {
			inner: RuntimeRegistry::new("hooks", builtins),
		}
	}

	pub fn get(&self, key: &str) -> Option<HooksRef> {
		self.inner.get(key)
	}

	pub fn all(&self) -> Vec<HooksRef> {
		self.inner.snapshot_guard().iter_refs().collect()
	}

	pub fn snapshot_guard(&self) -> crate::core::index::SnapshotGuard<HookEntry, HookId> {
		self.inner.snapshot_guard()
	}

	pub fn for_event(&self, event: HookEvent) -> Vec<HooksRef> {
		let snap = self.inner.snapshot();
		let mut refs = Vec::new();
		for (idx, entry) in snap.table.iter().enumerate() {
			if entry.event == event {
				refs.push(RegistryRef {
					snap: snap.clone(),
					id: HookId::from_u32(idx as u32),
				});
			}
		}
		refs
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	pub fn register(&self, def: &'static HookDef) -> bool {
		self.inner.register(def).is_ok()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::index::RegistryBuilder;
	use crate::core::{RegistryMetaStatic, RegistrySource};
	use crate::hooks::{HookAction, HookContext, HookHandler, HookInput, HookMutability, HookPriority, HookResult};

	fn test_hook(_ctx: &HookContext) -> HookAction {
		HookAction::Done(HookResult::Continue)
	}

	static RUNTIME_HOOK: HookDef = HookDef {
		meta: RegistryMetaStatic {
			id: "registry::hooks::runtime_test",
			name: "runtime_test",
			keys: &[],
			description: "runtime hook test",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
		event: crate::HookEvent::EditorTick,
		mutability: HookMutability::Immutable,
		execution_priority: HookPriority::Interactive,
		handler: HookHandler::Immutable(test_hook),
	};

	#[test]
	fn runtime_registration_is_visible_in_event_lookup() {
		let builder: RegistryBuilder<HookInput, HookEntry, HookId> = RegistryBuilder::new("hooks-test");
		let registry = HooksRegistry::new(builder.build());
		assert!(registry.register(&RUNTIME_HOOK));

		let hooks = registry.for_event(crate::HookEvent::EditorTick);
		assert_eq!(hooks.len(), 1);
		assert_eq!(hooks[0].id_str(), RUNTIME_HOOK.meta.id);
	}
}
