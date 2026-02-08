use std::sync::Arc;

use super::*;
use crate::kdl::types::TextObjectsBlob;
use crate::textobj::handler::TextObjectHandlerStatic;
use crate::textobj::{TextObjectEntry, TextObjectHandler};

/// A text object definition assembled from KDL metadata + Rust handlers.
#[derive(Clone)]
pub struct LinkedTextObjectDef {
	/// Canonical ID: `"xeno-registry::{name}"`.
	pub id: String,
	/// Text object name (linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Primary trigger character.
	pub trigger: char,
	/// Alternate trigger characters.
	pub alt_triggers: Vec<char>,
	/// Inner selection handler from Rust.
	pub inner: TextObjectHandler,
	/// Around selection handler from Rust.
	pub around: TextObjectHandler,
	/// Where this definition came from.
	pub source: RegistrySource,
}

impl BuildEntry<TextObjectEntry> for LinkedTextObjectDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: &self.id,
			name: &self.name,
			keys: StrListRef::Owned(&[]),
			description: &self.description,
			priority: 0,
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

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> TextObjectEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		TextObjectEntry {
			meta,
			trigger: self.trigger,
			alt_triggers: Arc::from(self.alt_triggers.as_slice()),
			inner: self.inner,
			around: self.around,
		}
	}
}

/// Parses a single-character trigger string into a `char`.
fn parse_trigger(s: &str, name: &str) -> char {
	let mut chars = s.chars();
	let c = chars
		.next()
		.unwrap_or_else(|| panic!("text object '{}' has empty trigger", name));
	assert!(
		chars.next().is_none(),
		"text object '{}' trigger '{}' is not a single character",
		name,
		s
	);
	c
}

/// Links KDL text object metadata with handler statics, producing `LinkedTextObjectDef`s.
///
/// Panics if any KDL text object has no matching handler, or vice versa.
pub fn link_text_objects(
	metadata: &TextObjectsBlob,
	handlers: impl Iterator<Item = &'static TextObjectHandlerStatic>,
) -> Vec<LinkedTextObjectDef> {
	let handler_map: HashMap<&str, &TextObjectHandlerStatic> =
		handlers.map(|h| (h.name, h)).collect();

	let mut defs = Vec::new();
	let mut used_handlers = HashSet::new();

	for meta in &metadata.text_objects {
		let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
			panic!(
				"KDL text object '{}' has no matching text_object_handler!() in Rust",
				meta.name
			)
		});
		used_handlers.insert(meta.name.as_str());

		let id = format!("xeno-registry::{}", meta.name);
		let trigger = parse_trigger(&meta.trigger, &meta.name);
		let alt_triggers: Vec<char> = meta
			.alt_triggers
			.iter()
			.map(|s| parse_trigger(s, &meta.name))
			.collect();

		defs.push(LinkedTextObjectDef {
			id,
			name: meta.name.clone(),
			description: meta.description.clone(),
			trigger,
			alt_triggers,
			inner: handler.inner,
			around: handler.around,
			source: RegistrySource::Crate(handler.crate_name),
		});
	}

	for name in handler_map.keys() {
		if !used_handlers.contains(name) {
			panic!(
				"text_object_handler!({}) has no matching entry in text_objects.kdl",
				name
			);
		}
	}

	defs
}
