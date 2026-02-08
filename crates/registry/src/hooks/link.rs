use super::spec::HooksSpec;
use crate::HookEvent;
use crate::core::{
	FrozenInterner, LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol,
};
use crate::hooks::handler::HookHandlerStatic;
use crate::hooks::{HookEntry, HookHandler, HookMutability, HookPriority};

pub type LinkedHookDef = LinkedDef<HookPayload>;

#[derive(Clone)]
pub struct HookPayload {
	pub event: HookEvent,
	pub mutability: HookMutability,
	pub execution_priority: HookPriority,
	pub handler: HookHandler,
}

impl LinkedPayload<HookEntry> for HookPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> HookEntry {
		HookEntry {
			meta,
			event: self.event,
			mutability: self.mutability,
			execution_priority: self.execution_priority,
			handler: self.handler,
		}
	}
}

pub fn link_hooks(
	spec: &HooksSpec,
	handlers: impl Iterator<Item = &'static HookHandlerStatic>,
) -> Vec<LinkedHookDef> {
	crate::defs::link::link_by_name(
		&spec.hooks,
		handlers,
		|m| m.common.name.as_str(),
		|h| h.name,
		|meta, handler| {
			let common = &meta.common;
			let id = format!("xeno-registry::{}", common.name);

			// Validate KDL event matches handler event
			let kdl_event = &meta.event;
			let handler_event_str = handler.handler.event.as_str();
			if kdl_event != handler_event_str {
				panic!(
					"hook '{}' event mismatch: KDL says '{}', handler says '{}' \
					(hint: hooks.kdl must use HookEvent::as_str() values, e.g. \"buffer:open\")",
					common.name, kdl_event, handler_event_str
				);
			}

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: common.name.clone(),
					keys: common.keys.clone(),
					description: common.description.clone(),
					priority: common.priority,
					source: RegistrySource::Crate(handler.crate_name),
					required_caps: vec![],
					flags: common.flags,
					short_desc: Some(
						common
							.short_desc
							.clone()
							.unwrap_or_else(|| common.description.clone()),
					),
				},
				payload: HookPayload {
					event: handler.handler.event,
					mutability: handler.handler.mutability,
					execution_priority: handler.handler.execution_priority,
					handler: handler.handler.handler,
				},
			}
		},
		"hook",
	)
}
