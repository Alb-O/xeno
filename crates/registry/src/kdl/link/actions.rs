use std::sync::Arc;

use super::*;
use crate::actions::def::ActionHandler;
use crate::actions::entry::ActionEntry;
use crate::actions::handler::ActionHandlerStatic;
use crate::actions::{KeyBindingDef, KeyPrefixDef};
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::{ActionMetaRaw, ActionsBlob, KeyBindingRaw};

/// An action definition assembled from KDL metadata + Rust handler.
pub type LinkedActionDef = LinkedDef<ActionPayload>;

#[derive(Clone)]
pub struct ActionPayload {
	pub handler: ActionHandler,
	pub bindings: Arc<[KeyBindingDef]>,
}

impl LinkedPayload<ActionEntry> for ActionPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		short_desc: Symbol,
	) -> ActionEntry {
		ActionEntry {
			meta,
			short_desc,
			handler: self.handler,
			bindings: Arc::clone(&self.bindings),
		}
	}
}

pub(crate) fn parse_bindings(raw: &[KeyBindingRaw], action_id: Arc<str>) -> Vec<KeyBindingDef> {
	raw.iter()
		.map(|b| KeyBindingDef {
			mode: super::parse::parse_binding_mode(&b.mode),
			keys: Arc::from(b.keys.as_str()),
			action: Arc::clone(&action_id),
			priority: 100,
		})
		.collect()
}

/// Links KDL metadata with handler statics, producing `LinkedActionDef`s.
///
/// Panics if any KDL action has no matching handler, or vice versa.
pub fn link_actions(
	metadata: &ActionsBlob,
	handlers: impl Iterator<Item = &'static ActionHandlerStatic>,
) -> Vec<LinkedActionDef> {
	super::spec::link_domain::<ActionLinkSpec>(&metadata.actions, handlers)
}

struct ActionLinkSpec;

impl super::spec::DomainLinkSpec for ActionLinkSpec {
	type Meta = ActionMetaRaw;
	type HandlerFn = ActionHandler;
	type Entry = ActionEntry;
	type Payload = ActionPayload;

	const WHAT: &'static str = "action";
	const CANONICAL_PREFIX: &'static str = "xeno-registry::";

	fn common(meta: &Self::Meta) -> &crate::kdl::types::MetaCommonRaw {
		&meta.common
	}

	fn build_payload(
		meta: &Self::Meta,
		handler: Self::HandlerFn,
		canonical_id: Arc<str>,
	) -> Self::Payload {
		let bindings = parse_bindings(&meta.bindings, canonical_id);
		ActionPayload {
			handler,
			bindings: Arc::from(bindings.into_boxed_slice()),
		}
	}
}

/// Parses prefix data from the blob into `KeyPrefixDef`s.
pub fn link_prefixes(metadata: &ActionsBlob) -> Vec<KeyPrefixDef> {
	metadata
		.prefixes
		.iter()
		.map(|p| KeyPrefixDef {
			mode: super::parse::parse_binding_mode(&p.mode),
			keys: Arc::from(p.keys.as_str()),
			description: Arc::from(p.description.as_str()),
		})
		.collect()
}
