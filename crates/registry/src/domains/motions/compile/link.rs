use super::spec::MotionsSpec;
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol};
use crate::motions::handler::MotionHandlerStatic;
use crate::motions::{MotionEntry, MotionHandler};

pub type LinkedMotionDef = LinkedDef<MotionPayload>;

#[derive(Clone)]
pub struct MotionPayload {
	pub handler: MotionHandler,
}

impl LinkedPayload<MotionEntry> for MotionPayload {
	fn build_entry(&self, _ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> MotionEntry {
		MotionEntry { meta, handler: self.handler }
	}
}

pub fn link_motions(spec: &MotionsSpec, handlers: impl Iterator<Item = &'static MotionHandlerStatic>) -> Vec<LinkedMotionDef> {
	crate::defs::link::link_by_name(
		&spec.motions,
		handlers,
		|m| m.common.name.as_str(),
		|h| h.name,
		|meta, handler| {
			let common = &meta.common;
			let id = format!("xeno-registry::{}", common.name);

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: common.name.clone(),
					keys: common.keys.clone(),
					description: common.description.clone(),
					priority: common.priority,
					source: RegistrySource::Crate(handler.crate_name),
					mutates_buffer: false,
					short_desc: Some(common.name.clone()),
				},
				payload: MotionPayload { handler: handler.handler },
			}
		},
		"motion",
	)
}
