use super::spec::StatuslineSpec;
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol};
use crate::statusline::handler::{StatuslineHandlerStatic, StatuslineRenderHandler};
use crate::statusline::{SegmentPosition, StatuslineEntry};

pub type LinkedStatuslineDef = LinkedDef<StatuslinePayload>;

#[derive(Clone)]
pub struct StatuslinePayload {
	pub position: SegmentPosition,
	pub default_enabled: bool,
	pub render: StatuslineRenderHandler,
}

impl LinkedPayload<StatuslineEntry> for StatuslinePayload {
	fn build_entry(&self, _ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> StatuslineEntry {
		StatuslineEntry {
			meta,
			position: self.position,
			default_enabled: self.default_enabled,
			render: self.render,
		}
	}
}

fn parse_position(s: &str, name: &str) -> SegmentPosition {
	match s {
		"left" => SegmentPosition::Left,
		"center" => SegmentPosition::Center,
		"right" => SegmentPosition::Right,
		other => panic!("unknown position '{}' for segment '{}'", other, name),
	}
}

pub fn link_statusline(spec: &StatuslineSpec, handlers: impl Iterator<Item = &'static StatuslineHandlerStatic>) -> Vec<LinkedStatuslineDef> {
	crate::defs::link::link_by_name(
		&spec.segments,
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
					flags: common.flags,
					short_desc: Some(common.name.clone()),
				},
				payload: StatuslinePayload {
					position: parse_position(&meta.position, &common.name),
					default_enabled: true,
					render: handler.handler,
				},
			}
		},
		"segment",
	)
}
