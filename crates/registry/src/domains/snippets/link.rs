use super::entry::SnippetEntry;
use super::spec::SnippetsSpec;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};

pub type LinkedSnippetDef = LinkedDef<SnippetPayload>;

#[derive(Clone)]
pub struct SnippetPayload {
	pub body: String,
}

impl LinkedPayload<SnippetEntry> for SnippetPayload {
	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.push(&self.body);
	}

	fn build_entry(&self, ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> SnippetEntry {
		SnippetEntry {
			meta,
			body: ctx.intern(&self.body),
		}
	}
}

pub fn link_snippets(spec: &SnippetsSpec) -> Vec<LinkedSnippetDef> {
	spec.snippets
		.iter()
		.map(|snippet| LinkedDef {
			meta: crate::defs::link::linked_meta_from_spec(&snippet.common),
			payload: SnippetPayload { body: snippet.body.clone() },
		})
		.collect()
}
