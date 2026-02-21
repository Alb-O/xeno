use super::spec::GuttersSpec;
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, RegistrySource, Symbol};
use crate::gutter::handler::{GutterHandlerStatic, GutterRenderHandler};
use crate::gutter::{GutterEntry, GutterWidth, GutterWidthContext};

pub type LinkedGutterDef = LinkedDef<GutterPayload>;

#[derive(Clone)]
pub struct GutterPayload {
	pub default_enabled: bool,
	pub width: GutterWidth,
	pub render: GutterRenderHandler,
}

impl LinkedPayload<GutterEntry> for GutterPayload {
	fn build_entry(&self, _ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> GutterEntry {
		GutterEntry {
			meta,
			default_enabled: self.default_enabled,
			width: self.width,
			render: self.render,
		}
	}
}

fn dynamic_width(ctx: &GutterWidthContext) -> u16 {
	(ctx.total_lines.max(1).ilog10() as u16 + 1).max(3)
}

fn parse_width(raw: &str, name: &str) -> GutterWidth {
	if raw == "dynamic" {
		return GutterWidth::Dynamic(dynamic_width);
	}
	match raw.parse::<u16>() {
		Ok(width) => GutterWidth::Fixed(width),
		Err(_) => panic!("unknown width '{}' for gutter '{}'", raw, name),
	}
}

pub fn link_gutters(spec: &GuttersSpec, handlers: impl Iterator<Item = &'static GutterHandlerStatic>) -> Vec<LinkedGutterDef> {
	crate::defs::link::link_by_name(
		&spec.gutters,
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
				payload: GutterPayload {
					default_enabled: meta.enabled,
					width: parse_width(&meta.width, &common.name),
					render: handler.handler,
				},
			}
		},
		"gutter",
	)
}
