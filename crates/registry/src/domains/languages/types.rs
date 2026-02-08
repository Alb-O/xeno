use std::sync::Arc;

use crate::core::{BuildEntry, FrozenInterner, RegistryMeta, RegistryMetaRef, StrListRef, Symbol};

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
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		""
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		if let Some(s) = self.scope {
			sink.push(s);
		}
		if let Some(s) = self.grammar_name {
			sink.push(s);
		}
		if let Some(s) = self.injection_regex {
			sink.push(s);
		}
		for s in self.extensions {
			sink.push(s);
		}
		for s in self.filenames {
			sink.push(s);
		}
		for s in self.globs {
			sink.push(s);
		}
		for s in self.shebangs {
			sink.push(s);
		}
		for s in self.comment_tokens {
			sink.push(s);
		}
		if let Some((s1, s2)) = self.block_comment {
			sink.push(s1);
			sink.push(s2);
		}
		for s in self.lsp_servers {
			sink.push(s);
		}
		for s in self.roots {
			sink.push(s);
		}
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> LanguageEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);
		let intern = |s: &str| interner.get(s).expect("missing interned string");
		let intern_slice = |xs: &[&str]| xs.iter().map(|s| intern(s)).collect::<Vec<_>>().into();

		LanguageEntry {
			meta,
			scope: self.scope.map(intern),
			grammar_name: self.grammar_name.map(intern),
			injection_regex: self.injection_regex.map(intern),
			auto_format: self.auto_format,
			extensions: intern_slice(self.extensions),
			filenames: intern_slice(self.filenames),
			globs: intern_slice(self.globs),
			shebangs: intern_slice(self.shebangs),
			comment_tokens: intern_slice(self.comment_tokens),
			block_comment: self.block_comment.map(|(s1, s2)| (intern(s1), intern(s2))),
			lsp_servers: intern_slice(self.lsp_servers),
			roots: intern_slice(self.roots),
		}
	}
}

pub type LanguageInput =
	crate::core::def_input::DefInput<LanguageDef, super::link::LinkedLanguageDef>;
