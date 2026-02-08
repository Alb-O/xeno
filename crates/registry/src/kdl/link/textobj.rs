use std::sync::Arc;

use super::*;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::{TextObjectMetaRaw, TextObjectsBlob};
use crate::textobj::handler::{TextObjectHandlerStatic, TextObjectHandlers};
use crate::textobj::{TextObjectEntry, TextObjectHandler};

/// A text object definition assembled from KDL metadata + Rust handlers.
pub type LinkedTextObjectDef = LinkedDef<TextObjectPayload>;

#[derive(Clone)]
pub struct TextObjectPayload {
	pub trigger: char,
	pub alt_triggers: Vec<char>,
	pub inner: TextObjectHandler,
	pub around: TextObjectHandler,
}

impl LinkedPayload<TextObjectEntry> for TextObjectPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> TextObjectEntry {
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

/// Links KDL text object metadata with handler statics.
pub fn link_text_objects(
	metadata: &TextObjectsBlob,
	handlers: impl Iterator<Item = &'static TextObjectHandlerStatic>,
) -> Vec<LinkedTextObjectDef> {
	super::spec::link_domain::<TextObjectLinkSpec>(&metadata.text_objects, handlers)
}

struct TextObjectLinkSpec;

impl super::spec::DomainLinkSpec for TextObjectLinkSpec {
	type Meta = TextObjectMetaRaw;
	type HandlerFn = TextObjectHandlers;
	type Entry = TextObjectEntry;
	type Payload = TextObjectPayload;

	const WHAT: &'static str = "text_object";
	const CANONICAL_PREFIX: &'static str = "xeno-registry::";

	fn common(meta: &Self::Meta) -> &crate::kdl::types::MetaCommonRaw {
		&meta.common
	}

	fn build_payload(
		meta: &Self::Meta,
		handler: Self::HandlerFn,
		_canonical_id: Arc<str>,
	) -> Self::Payload {
		let trigger = parse_trigger(&meta.trigger, &meta.common.name);
		let alt_triggers: Vec<char> = meta
			.alt_triggers
			.iter()
			.map(|s| parse_trigger(s, &meta.common.name))
			.collect();

		TextObjectPayload {
			trigger,
			alt_triggers,
			inner: handler.inner,
			around: handler.around,
		}
	}
}
