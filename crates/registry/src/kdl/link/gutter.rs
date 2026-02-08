use super::*;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::gutter::handler::{GutterHandlerStatic, GutterRenderHandler};
use crate::gutter::{GutterEntry, GutterWidth, GutterWidthContext};
use crate::kdl::types::{GutterMetaRaw, GuttersBlob};

/// A gutter definition assembled from KDL metadata + Rust handlers.
pub type LinkedGutterDef = LinkedDef<GutterPayload>;

#[derive(Clone)]
pub struct GutterPayload {
	pub default_enabled: bool,
	pub width: GutterWidth,
	pub render: GutterRenderHandler,
}

impl LinkedPayload<GutterEntry> for GutterPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> GutterEntry {
		GutterEntry {
			meta,
			default_enabled: self.default_enabled,
			width: self.width,
			render: self.render,
		}
	}
}

/// Links KDL gutter metadata with handler statics.
pub fn link_gutters(
	metadata: &GuttersBlob,
	handlers: impl Iterator<Item = &'static GutterHandlerStatic>,
) -> Vec<LinkedGutterDef> {
	super::spec::link_domain::<GutterLinkSpec>(&metadata.gutters, handlers)
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

struct GutterLinkSpec;

impl super::spec::DomainLinkSpec for GutterLinkSpec {
	type Meta = GutterMetaRaw;
	type HandlerFn = GutterRenderHandler;
	type Entry = GutterEntry;
	type Payload = GutterPayload;

	const WHAT: &'static str = "gutter";
	const CANONICAL_PREFIX: &'static str = "xeno-registry::";

	fn common(meta: &Self::Meta) -> &crate::kdl::types::MetaCommonRaw {
		&meta.common
	}

	fn short_desc(meta: &Self::Meta) -> String {
		meta.common.name.clone()
	}

	fn build_payload(
		meta: &Self::Meta,
		handler: Self::HandlerFn,
		_canonical_id: std::sync::Arc<str>,
	) -> Self::Payload {
		GutterPayload {
			default_enabled: meta.enabled,
			width: parse_width(&meta.width, &meta.common.name),
			render: handler,
		}
	}
}
