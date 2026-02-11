use std::sync::Arc;

use super::spec::{ActionsSpec, KeyBindingSpec};
use crate::actions::def::ActionHandler;
use crate::actions::entry::ActionEntry;
use crate::actions::handler::ActionHandlerStatic;
use crate::actions::{BindingMode, KeyBindingDef, KeyPrefixDef};
use crate::core::{Capability, LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol};

/// An action definition assembled from spec + Rust handler.
pub type LinkedActionDef = LinkedDef<ActionPayload>;

#[derive(Clone)]
pub struct ActionPayload {
	pub handler: ActionHandler,
	pub bindings: Arc<[KeyBindingDef]>,
}

impl LinkedPayload<ActionEntry> for ActionPayload {
	fn build_entry(&self, _ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, short_desc: Symbol) -> ActionEntry {
		ActionEntry {
			meta,
			short_desc,
			handler: self.handler,
			bindings: Arc::clone(&self.bindings),
		}
	}
}

pub(crate) fn parse_bindings(raw: &[KeyBindingSpec], action_id: Arc<str>) -> Vec<KeyBindingDef> {
	raw.iter()
		.map(|b| KeyBindingDef {
			mode: parse_binding_mode(&b.mode),
			keys: Arc::from(b.keys.as_str()),
			action: Arc::clone(&action_id),
			priority: 100,
		})
		.collect()
}

pub(crate) fn parse_binding_mode(mode: &str) -> BindingMode {
	match mode {
		"normal" => BindingMode::Normal,
		"insert" => BindingMode::Insert,
		"match" => BindingMode::Match,
		"space" => BindingMode::Space,
		other => panic!("unknown binding mode: {}", other),
	}
}

pub(crate) fn parse_capability(name: &str) -> Capability {
	match name {
		"Text" => Capability::Text,
		"Cursor" => Capability::Cursor,
		"Selection" => Capability::Selection,
		"Mode" => Capability::Mode,
		"Messaging" => Capability::Messaging,
		"Edit" => Capability::Edit,
		"Search" => Capability::Search,
		"Undo" => Capability::Undo,
		"FileOps" => Capability::FileOps,
		"Overlay" => Capability::Overlay,
		other => panic!("unknown capability: {}", other),
	}
}

/// Links spec with handler statics, producing `LinkedActionDef`s.
pub fn link_actions(spec: &ActionsSpec, handlers: impl Iterator<Item = &'static ActionHandlerStatic>) -> Vec<LinkedActionDef> {
	crate::defs::link::link_by_name(
		&spec.actions,
		handlers,
		|m| m.common.name.as_str(),
		|h| h.name,
		|meta, handler| {
			let common = &meta.common;
			let id = format!("xeno-registry::{}", common.name);
			let canonical_id: Arc<str> = Arc::from(id.as_str());

			let required_caps = common.caps.iter().map(|c| parse_capability(c)).collect();

			let bindings = parse_bindings(&meta.bindings, canonical_id.clone());

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: common.name.clone(),
					keys: common.keys.clone(),
					description: common.description.clone(),
					priority: common.priority,
					source: RegistrySource::Crate(handler.crate_name),
					required_caps,
					flags: common.flags,
					short_desc: Some(common.short_desc.clone().unwrap_or_else(|| common.description.clone())),
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

/// Parses prefix data from the spec into `KeyPrefixDef`s.
pub fn link_prefixes(spec: &ActionsSpec) -> Vec<KeyPrefixDef> {
	spec.prefixes
		.iter()
		.map(|p| KeyPrefixDef {
			mode: parse_binding_mode(&p.mode),
			keys: Arc::from(p.keys.as_str()),
			description: Arc::from(p.description.as_str()),
		})
		.collect()
}
