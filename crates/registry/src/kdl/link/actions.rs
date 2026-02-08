use std::sync::Arc;

use super::*;
use crate::actions::def::ActionHandler;
use crate::actions::entry::ActionEntry;
use crate::actions::handler::ActionHandlerStatic;
use crate::actions::{KeyBindingDef, KeyPrefixDef};
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::{ActionsBlob, KeyBindingRaw};

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
	super::common::link_by_name(
		&metadata.actions,
		handlers,
		|m| &m.name,
		|h| h.name,
		|meta, handler| {
			let id = format!("xeno-registry::{}", meta.name);
			let action_id: Arc<str> = Arc::from(id.as_str());
			let short_desc = meta
				.short_desc
				.clone()
				.unwrap_or_else(|| meta.description.clone());
			let caps = meta
				.caps
				.iter()
				.map(|c| super::parse::parse_capability(c))
				.collect();
			let bindings = parse_bindings(&meta.bindings, action_id);

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: meta.name.clone(),
					keys: meta.keys.clone(),
					description: meta.description.clone(),
					priority: meta.priority,
					source: RegistrySource::Crate(handler.crate_name),
					required_caps: caps,
					flags: meta.flags,
					short_desc: Some(short_desc),
				},
				payload: ActionPayload {
					handler: handler.handler,
					bindings: Arc::from(bindings.into_boxed_slice()),
				},
			}
		},
		"action",
	)
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
