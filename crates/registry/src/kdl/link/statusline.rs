use super::*;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::{StatuslineBlob, StatuslineMetaRaw};
use crate::statusline::handler::{StatuslineHandlerStatic, StatuslineRenderHandler};
use crate::statusline::{SegmentPosition, StatuslineEntry};

/// A statusline definition assembled from KDL metadata + Rust handler.
pub type LinkedStatuslineDef = LinkedDef<StatuslinePayload>;

#[derive(Clone)]
pub struct StatuslinePayload {
	pub position: SegmentPosition,
	pub default_enabled: bool,
	pub render: StatuslineRenderHandler,
}

impl LinkedPayload<StatuslineEntry> for StatuslinePayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> StatuslineEntry {
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

/// Links KDL statusline metadata with handler statics.
pub fn link_statusline(
	metadata: &StatuslineBlob,
	handlers: impl Iterator<Item = &'static StatuslineHandlerStatic>,
) -> Vec<LinkedStatuslineDef> {
	super::spec::link_domain::<StatuslineLinkSpec>(&metadata.segments, handlers)
}

struct StatuslineLinkSpec;

impl super::spec::DomainLinkSpec for StatuslineLinkSpec {
	type Meta = StatuslineMetaRaw;
	type HandlerFn = StatuslineRenderHandler;
	type Entry = StatuslineEntry;
	type Payload = StatuslinePayload;

	const WHAT: &'static str = "segment";
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
		StatuslinePayload {
			position: parse_position(&meta.position, &meta.common.name),
			default_enabled: true,
			render: handler,
		}
	}
}
