use super::spec::LanguagesSpec;
use super::types::LanguageEntry;
use crate::core::{FrozenInterner, LinkedDef, LinkedPayload, RegistryMeta, Symbol};

pub type LinkedLanguageDef = LinkedDef<LanguagePayload>;

#[derive(Clone)]
pub struct LanguagePayload {
	pub scope: Option<String>,
	pub grammar_name: Option<String>,
	pub injection_regex: Option<String>,
	pub auto_format: bool,
	pub extensions: Vec<String>,
	pub filenames: Vec<String>,
	pub globs: Vec<String>,
	pub shebangs: Vec<String>,
	pub comment_tokens: Vec<String>,
	pub block_comment: Option<(String, String)>,
	pub lsp_servers: Vec<String>,
	pub roots: Vec<String>,
}

impl LinkedPayload<LanguageEntry> for LanguagePayload {
	fn collect_payload_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		if let Some(s) = self.scope.as_deref() {
			sink.push(s);
		}
		if let Some(s) = self.grammar_name.as_deref() {
			sink.push(s);
		}
		if let Some(s) = self.injection_regex.as_deref() {
			sink.push(s);
		}
		for s in &self.extensions {
			sink.push(s);
		}
		for s in &self.filenames {
			sink.push(s);
		}
		for s in &self.globs {
			sink.push(s);
		}
		for s in &self.shebangs {
			sink.push(s);
		}
		for s in &self.comment_tokens {
			sink.push(s);
		}
		if let Some((s1, s2)) = self.block_comment.as_ref() {
			sink.push(s1);
			sink.push(s2);
		}
		for s in &self.lsp_servers {
			sink.push(s);
		}
		for s in &self.roots {
			sink.push(s);
		}
	}

	fn build_entry(
		&self,
		interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> LanguageEntry {
		let intern = |s: &str| interner.get(s).expect("missing interned string");
		let intern_opt = |s: &Option<String>| s.as_deref().map(intern);
		let intern_slice = |xs: &[String]| xs.iter().map(|s| intern(s)).collect::<Vec<_>>().into();

		LanguageEntry {
			meta,
			scope: intern_opt(&self.scope),
			grammar_name: intern_opt(&self.grammar_name),
			injection_regex: intern_opt(&self.injection_regex),
			auto_format: self.auto_format,
			extensions: intern_slice(&self.extensions),
			filenames: intern_slice(&self.filenames),
			globs: intern_slice(&self.globs),
			shebangs: intern_slice(&self.shebangs),
			comment_tokens: intern_slice(&self.comment_tokens),
			block_comment: self
				.block_comment
				.as_ref()
				.map(|(s1, s2)| (intern(s1), intern(s2))),
			lsp_servers: intern_slice(&self.lsp_servers),
			roots: intern_slice(&self.roots),
		}
	}
}

pub fn link_languages(spec: &LanguagesSpec) -> Vec<LinkedLanguageDef> {
	spec.langs
		.iter()
		.map(|l| LinkedDef {
			meta: crate::defs::link::linked_meta_from_spec(&l.common),
			payload: LanguagePayload {
				scope: l.scope.clone(),
				grammar_name: l.grammar_name.clone(),
				injection_regex: l.injection_regex.clone(),
				auto_format: l.auto_format,
				extensions: l.extensions.clone(),
				filenames: l.filenames.clone(),
				globs: l.globs.clone(),
				shebangs: l.shebangs.clone(),
				comment_tokens: l.comment_tokens.clone(),
				block_comment: l.block_comment.clone(),
				lsp_servers: l.lsp_servers.clone(),
				roots: l.roots.clone(),
			},
		})
		.collect()
}
