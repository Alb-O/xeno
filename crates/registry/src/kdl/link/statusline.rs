use super::*;
use crate::kdl::types::StatuslineBlob;
use crate::statusline::handler::StatuslineHandlerStatic;
use crate::statusline::{RenderedSegment, SegmentPosition, StatuslineContext, StatuslineEntry};

/// A statusline segment definition assembled from KDL metadata + Rust handler.
#[derive(Clone)]
pub struct LinkedStatuslineDef {
	pub id: String,
	pub name: String,
	pub description: String,
	pub priority: i16,
	pub position: SegmentPosition,
	pub default_enabled: bool,
	pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
	pub source: RegistrySource,
}

fn parse_position(s: &str, name: &str) -> SegmentPosition {
	match s {
		"left" => SegmentPosition::Left,
		"center" => SegmentPosition::Center,
		"right" => SegmentPosition::Right,
		other => panic!("unknown position '{}' for segment '{}'", other, name),
	}
}

impl BuildEntry<StatuslineEntry> for LinkedStatuslineDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: &self.id,
			name: &self.name,
			keys: StrListRef::Owned(&[]),
			description: &self.description,
			priority: self.priority,
			source: self.source,
			required_caps: &[],
			flags: 0,
		}
	}

	fn short_desc_str(&self) -> &str {
		&self.name
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> StatuslineEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		StatuslineEntry {
			meta,
			position: self.position,
			default_enabled: self.default_enabled,
			render: self.render,
		}
	}
}

/// Links KDL statusline metadata with handler statics.
pub fn link_statusline(
	metadata: &StatuslineBlob,
	handlers: impl Iterator<Item = &'static StatuslineHandlerStatic>,
) -> Vec<LinkedStatuslineDef> {
	let handler_map: HashMap<&str, &StatuslineHandlerStatic> =
		handlers.map(|h| (h.name, h)).collect();

	let mut defs = Vec::new();
	let mut used_handlers = HashSet::new();

	for meta in &metadata.segments {
		let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
			panic!(
				"KDL segment '{}' has no matching segment_handler!() in Rust",
				meta.name
			)
		});
		used_handlers.insert(meta.name.as_str());

		let id = format!("xeno-registry::{}", meta.name);
		let position = parse_position(&meta.position, &meta.name);

		defs.push(LinkedStatuslineDef {
			id,
			name: meta.name.clone(),
			description: meta.description.clone(),
			priority: meta.priority,
			position,
			default_enabled: true,
			render: handler.render,
			source: RegistrySource::Crate(handler.crate_name),
		});
	}

	for name in handler_map.keys() {
		if !used_handlers.contains(name) {
			panic!(
				"segment_handler!({}) has no matching entry in statusline.kdl",
				name
			);
		}
	}

	defs
}
