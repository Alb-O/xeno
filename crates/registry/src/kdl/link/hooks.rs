use std::sync::Arc;

use super::*;
use crate::HookEvent;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::hooks::handler::{HookHandlerConfig, HookHandlerStatic};
use crate::hooks::{HookEntry, HookHandler, HookMutability, HookPriority};
use crate::kdl::types::{HookMetaRaw, HooksBlob};

/// A hook definition assembled from KDL metadata + Rust handler.
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

/// Links KDL hook metadata with handler statics.
pub fn link_hooks(
	metadata: &HooksBlob,
	handlers: impl Iterator<Item = &'static HookHandlerStatic>,
) -> Vec<LinkedHookDef> {
	super::spec::link_domain::<HookLinkSpec>(&metadata.hooks, handlers)
}

struct HookLinkSpec;

impl super::spec::DomainLinkSpec for HookLinkSpec {
	type Meta = HookMetaRaw;
	type HandlerFn = HookHandlerConfig;
	type Entry = HookEntry;
	type Payload = HookPayload;

	const WHAT: &'static str = "hook";
	const CANONICAL_PREFIX: &'static str = "xeno-registry::";

	fn common(meta: &Self::Meta) -> &crate::kdl::types::MetaCommonRaw {
		&meta.common
	}

	fn build_payload(
		meta: &Self::Meta,
		handler: Self::HandlerFn,
		_canonical_id: Arc<str>,
	) -> Self::Payload {
		// Validate KDL event matches handler event (fail-fast, deterministic)
		let kdl_event = &meta.event;
		let handler_event_str = handler.event.as_str();
		if kdl_event != handler_event_str {
			panic!(
				"hook '{}' event mismatch: KDL says '{}', handler says '{}' \
				(hint: hooks.kdl must use HookEvent::as_str() values, e.g. \"buffer:open\")",
				meta.common.name, kdl_event, handler_event_str
			);
		}

		HookPayload {
			event: handler.event,
			mutability: handler.mutability,
			execution_priority: handler.execution_priority,
			handler: handler.handler,
		}
	}
}
