use super::entry::SnippetEntry;
use crate::core::index::{BuildCtxExt, BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{RegistryMetaStatic, Symbol};

#[derive(Clone)]
pub struct SnippetDef {
	pub meta: RegistryMetaStatic,
	pub body: &'static str,
}

impl BuildEntry<SnippetEntry> for SnippetDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			mutates_buffer: self.meta.mutates_buffer,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.push(self.body);
	}

	fn build(&self, ctx: &mut dyn crate::core::index::BuildCtx, key_pool: &mut Vec<Symbol>) -> SnippetEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		SnippetEntry {
			meta,
			body: ctx.intern_req(self.body, "snippet body"),
		}
	}
}

pub type SnippetInput = crate::core::def_input::DefInput<SnippetDef, crate::snippets::link::LinkedSnippetDef>;
