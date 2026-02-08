use super::*;
use crate::HookEvent;
use crate::hooks::handler::HookHandlerStatic;
use crate::hooks::{HookEntry, HookHandler, HookMutability, HookPriority};
use crate::kdl::types::HooksBlob;

/// A hook definition assembled from KDL metadata + Rust handler.
#[derive(Clone)]
pub struct LinkedHookDef {
	pub id: String,
	pub name: String,
	pub description: String,
	pub priority: i16,
	pub event: HookEvent,
	pub mutability: HookMutability,
	pub execution_priority: HookPriority,
	pub handler: HookHandler,
	pub source: RegistrySource,
}

impl BuildEntry<HookEntry> for LinkedHookDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: &self.id,
			name: &self.name,
			keys: StrListRef::Owned(&[]),
			description: &self.description,
			priority: self.priority,
			source: self.source,
			required_caps: &[],
			flags: 0,
		}
	}

	fn short_desc_str(&self) -> &str {
		&self.name
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

/// Links KDL hook metadata with handler statics.
pub fn link_hooks(
	metadata: &HooksBlob,
	handlers: impl Iterator<Item = &'static HookHandlerStatic>,
) -> Vec<LinkedHookDef> {
	let handler_map: HashMap<&str, &HookHandlerStatic> = handlers.map(|h| (h.name, h)).collect();

	let mut defs = Vec::new();
	let mut used_handlers = HashSet::new();

	for meta in &metadata.hooks {
		let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
			panic!(
				"KDL hook '{}' has no matching hook_handler!() in Rust",
				meta.name
			)
		});
		used_handlers.insert(meta.name.as_str());

		let id = format!("xeno-registry::{}", meta.name);

		defs.push(LinkedHookDef {
			id,
			name: meta.name.clone(),
			description: meta.description.clone(),
			priority: meta.priority,
			event: handler.event,
			mutability: handler.mutability,
			execution_priority: handler.execution_priority,
			handler: handler.handler,
			source: RegistrySource::Crate(handler.crate_name),
		});
	}

	for name in handler_map.keys() {
		if !used_handlers.contains(name) {
			panic!("hook_handler!({}) has no matching entry in hooks.kdl", name);
		}
	}

	defs
}
