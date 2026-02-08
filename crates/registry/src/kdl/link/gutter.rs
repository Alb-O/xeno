use super::*;
use crate::gutter::handler::GutterHandlerStatic;
use crate::gutter::{GutterCell, GutterEntry, GutterLineContext, GutterWidth};
use crate::kdl::types::GuttersBlob;

/// A gutter definition assembled from KDL metadata + Rust handlers.
#[derive(Clone)]
pub struct LinkedGutterDef {
	pub id: String,
	pub name: String,
	pub description: String,
	pub priority: i16,
	pub default_enabled: bool,
	pub width: GutterWidth,
	pub render: fn(&GutterLineContext) -> Option<GutterCell>,
	pub source: RegistrySource,
}

impl BuildEntry<GutterEntry> for LinkedGutterDef {
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

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> GutterEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
	let handler_map: HashMap<&str, &GutterHandlerStatic> = handlers.map(|h| (h.name, h)).collect();

	let mut defs = Vec::new();
	let mut used_handlers = HashSet::new();

	for meta in &metadata.gutters {
		let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
			panic!(
				"KDL gutter '{}' has no matching gutter_handler!() in Rust",
				meta.name
			)
		});
		used_handlers.insert(meta.name.as_str());

		let id = format!("xeno-registry::{}", meta.name);

		defs.push(LinkedGutterDef {
			id,
			name: meta.name.clone(),
			description: meta.description.clone(),
			priority: meta.priority,
			default_enabled: meta.enabled,
			width: handler.width,
			render: handler.render,
			source: RegistrySource::Crate(handler.crate_name),
		});
	}

	for name in handler_map.keys() {
		if !used_handlers.contains(name) {
			panic!(
				"gutter_handler!({}) has no matching entry in gutters.kdl",
				name
			);
		}
	}

	defs
}
