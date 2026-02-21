use std::sync::Arc;

use crate::core::{BuildEntry, RegistryMeta, RegistryMetaRef, StrListRef, Symbol};

#[derive(Clone)]
pub struct LanguageQueryEntry {
	pub kind: Symbol,
	pub text: Symbol,
}

#[derive(Clone)]
pub enum ViewportRepairRuleEntry {
	/// e.g. /* ... */
	BlockComment { open: Symbol, close: Symbol, nestable: bool },

	/// e.g. "..." or '...'
	String { quote: Symbol, escape: Option<Symbol> },

	/// e.g. //
	LineComment { start: Symbol },
}

#[derive(Clone)]
pub struct ViewportRepairEntry {
	pub enabled: bool,
	pub max_scan_bytes: u32,
	pub prefer_real_closer: bool,
	pub max_forward_search_bytes: u32,
	pub rules: Arc<[ViewportRepairRuleEntry]>,
}

#[derive(Clone)]
pub struct LanguageEntry {
	pub meta: RegistryMeta,
	pub scope: Option<Symbol>,
	pub grammar_name: Option<Symbol>,
	pub injection_regex: Option<Symbol>,
	pub auto_format: bool,
	pub extensions: Arc<[Symbol]>,
	pub filenames: Arc<[Symbol]>,
	pub globs: Arc<[Symbol]>,
	pub shebangs: Arc<[Symbol]>,
	pub comment_tokens: Arc<[Symbol]>,
	pub block_comment: Option<(Symbol, Symbol)>,
	pub lsp_servers: Arc<[Symbol]>,
	pub roots: Arc<[Symbol]>,
	pub viewport_repair: Option<ViewportRepairEntry>,
	pub queries: Arc<[LanguageQueryEntry]>,
}

crate::impl_registry_entry!(LanguageEntry);

#[derive(Clone)]
pub struct LanguageDef {
	pub meta: crate::core::RegistryMetaStatic,
	pub scope: Option<&'static str>,
	pub grammar_name: Option<&'static str>,
	pub injection_regex: Option<&'static str>,
	pub auto_format: bool,
	pub extensions: &'static [&'static str],
	pub filenames: &'static [&'static str],
	pub globs: &'static [&'static str],
	pub shebangs: &'static [&'static str],
	pub comment_tokens: &'static [&'static str],
	pub block_comment: Option<(&'static str, &'static str)>,
	pub lsp_servers: &'static [&'static str],
	pub roots: &'static [&'static str],
}

impl BuildEntry<LanguageEntry> for LanguageDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			mutates_buffer: self.meta.mutates_buffer,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		""
	}

	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.opt(self.scope);
		collector.opt(self.grammar_name);
		collector.opt(self.injection_regex);
		collector.extend(self.extensions.iter().copied());
		collector.extend(self.filenames.iter().copied());
		collector.extend(self.globs.iter().copied());
		collector.extend(self.shebangs.iter().copied());
		collector.extend(self.comment_tokens.iter().copied());
		if let Some((s1, s2)) = self.block_comment {
			collector.push(s1);
			collector.push(s2);
		}
		collector.extend(self.lsp_servers.iter().copied());
		collector.extend(self.roots.iter().copied());
		// Static defs don't have queries or viewport_repair usually
	}

	fn build(&self, ctx: &mut dyn crate::core::index::BuildCtx, key_pool: &mut Vec<Symbol>) -> LanguageEntry {
		use crate::core::index::BuildCtxExt;

		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		LanguageEntry {
			meta,
			scope: self.scope.map(|s| ctx.intern(s)),
			grammar_name: self.grammar_name.map(|s| ctx.intern(s)),
			injection_regex: self.injection_regex.map(|s| ctx.intern(s)),
			auto_format: self.auto_format,
			extensions: ctx.intern_slice(self.extensions),
			filenames: ctx.intern_slice(self.filenames),
			globs: ctx.intern_slice(self.globs),
			shebangs: ctx.intern_slice(self.shebangs),
			comment_tokens: ctx.intern_slice(self.comment_tokens),
			block_comment: self.block_comment.map(|(s1, s2)| (ctx.intern(s1), ctx.intern(s2))),
			lsp_servers: ctx.intern_slice(self.lsp_servers),
			roots: ctx.intern_slice(self.roots),
			viewport_repair: None,
			queries: Arc::new([]),
		}
	}
}

pub type LanguageInput = crate::core::def_input::DefInput<LanguageDef, super::link::LinkedLanguageDef>;
