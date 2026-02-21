use std::sync::Arc;

use super::spec::TextObjectsSpec;
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol};
use crate::textobj::handler::TextObjectHandlerStatic;
use crate::textobj::{TextObjectEntry, TextObjectHandler};

pub type LinkedTextObjectDef = LinkedDef<TextObjectPayload>;

#[derive(Clone)]
pub struct TextObjectPayload {
	pub trigger: char,
	pub alt_triggers: Vec<char>,
	pub inner: TextObjectHandler,
	pub around: TextObjectHandler,
}

impl LinkedPayload<TextObjectEntry> for TextObjectPayload {
	fn build_entry(&self, _ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> TextObjectEntry {
		TextObjectEntry {
			meta,
			trigger: self.trigger,
			alt_triggers: Arc::from(self.alt_triggers.as_slice()),
			inner: self.inner,
			around: self.around,
		}
	}
}

fn parse_trigger(s: &str, name: &str) -> char {
	let mut chars = s.chars();
	let c = chars.next().unwrap_or_else(|| panic!("text object '{}' has empty trigger", name));
	assert!(chars.next().is_none(), "text object '{}' trigger '{}' is not a single character", name, s);
	c
}

pub fn link_text_objects(spec: &TextObjectsSpec, handlers: impl Iterator<Item = &'static TextObjectHandlerStatic>) -> Vec<LinkedTextObjectDef> {
	crate::defs::link::link_by_name(
		&spec.text_objects,
		handlers,
		|m| m.common.name.as_str(),
		|h| h.name,
		|meta, handler| {
			let common = &meta.common;
			let id = format!("xeno-registry::{}", common.name);

			let trigger = parse_trigger(&meta.trigger, &common.name);
			let alt_triggers: Vec<char> = meta.alt_triggers.iter().map(|s| parse_trigger(s, &common.name)).collect();

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: common.name.clone(),
					keys: common.keys.clone(),
					description: common.description.clone(),
					priority: common.priority,
					source: RegistrySource::Crate(handler.crate_name),
					mutates_buffer: false,
					short_desc: Some(common.short_desc.clone().unwrap_or_else(|| common.description.clone())),
				},
				payload: TextObjectPayload {
					trigger,
					alt_triggers,
					inner: handler.handler.inner,
					around: handler.handler.around,
				},
			}
		},
		"text_object",
	)
}
