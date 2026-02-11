use super::spec::LanguagesSpec;
use super::types::LanguageEntry;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};

pub type LinkedLanguageDef = LinkedDef<LanguagePayload>;

#[derive(Clone)]
pub struct LanguageQueryPayload {
	pub kind: String,
	pub text: String,
}

#[derive(Clone)]
pub enum ViewportRepairRulePayload {
	/// e.g. /* ... */
	BlockComment { open: String, close: String, nestable: bool },

	/// e.g. "..." or '...'
	String { quote: String, escape: Option<String> },

	/// e.g. //
	LineComment { start: String },
}

#[derive(Clone)]
pub struct ViewportRepairPayload {
	pub enabled: bool,
	pub max_scan_bytes: u32,
	pub prefer_real_closer: bool,
	pub max_forward_search_bytes: u32,
	pub rules: Vec<ViewportRepairRulePayload>,
}

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
	pub viewport_repair: Option<ViewportRepairPayload>,
	pub queries: Vec<LanguageQueryPayload>,
}

impl LinkedPayload<LanguageEntry> for LanguagePayload {
	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.opt(self.scope.as_deref());
		collector.opt(self.grammar_name.as_deref());
		collector.opt(self.injection_regex.as_deref());
		collector.extend(self.extensions.iter().map(|s| s.as_str()));
		collector.extend(self.filenames.iter().map(|s| s.as_str()));
		collector.extend(self.globs.iter().map(|s| s.as_str()));
		collector.extend(self.shebangs.iter().map(|s| s.as_str()));
		collector.extend(self.comment_tokens.iter().map(|s| s.as_str()));
		if let Some((s1, s2)) = self.block_comment.as_ref() {
			collector.push(s1);
			collector.push(s2);
		}
		collector.extend(self.lsp_servers.iter().map(|s| s.as_str()));
		collector.extend(self.roots.iter().map(|s| s.as_str()));
		if let Some(repair) = &self.viewport_repair {
			for rule in &repair.rules {
				match rule {
					ViewportRepairRulePayload::BlockComment { open, close, .. } => {
						collector.push(open);
						collector.push(close);
					}
					ViewportRepairRulePayload::String { quote, escape } => {
						collector.push(quote);
						collector.opt(escape.as_deref());
					}
					ViewportRepairRulePayload::LineComment { start } => {
						collector.push(start);
					}
				}
			}
		}
		for q in &self.queries {
			collector.push(&q.kind);
			collector.push(&q.text);
		}
	}

	fn build_entry(&self, ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> LanguageEntry {
		LanguageEntry {
			meta,
			scope: self.scope.as_ref().map(|s| ctx.intern(s)),
			grammar_name: self.grammar_name.as_ref().map(|s| ctx.intern(s)),
			injection_regex: self.injection_regex.as_ref().map(|s| ctx.intern(s)),
			auto_format: self.auto_format,
			extensions: self.extensions.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			filenames: self.filenames.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			globs: self.globs.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			shebangs: self.shebangs.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			comment_tokens: self.comment_tokens.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			block_comment: self.block_comment.as_ref().map(|(s1, s2)| (ctx.intern(s1), ctx.intern(s2))),
			lsp_servers: self.lsp_servers.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			roots: self.roots.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			viewport_repair: self.viewport_repair.as_ref().map(|r| super::types::ViewportRepairEntry {
				enabled: r.enabled,
				max_scan_bytes: r.max_scan_bytes,
				prefer_real_closer: r.prefer_real_closer,
				max_forward_search_bytes: r.max_forward_search_bytes,
				rules: r
					.rules
					.iter()
					.map(|rule| match rule {
						ViewportRepairRulePayload::BlockComment { open, close, nestable } => super::types::ViewportRepairRuleEntry::BlockComment {
							open: ctx.intern(open),
							close: ctx.intern(close),
							nestable: *nestable,
						},
						ViewportRepairRulePayload::String { quote, escape } => super::types::ViewportRepairRuleEntry::String {
							quote: ctx.intern(quote),
							escape: escape.as_ref().map(|s| ctx.intern(s)),
						},
						ViewportRepairRulePayload::LineComment { start } => super::types::ViewportRepairRuleEntry::LineComment { start: ctx.intern(start) },
					})
					.collect::<Vec<_>>()
					.into(),
			}),
			queries: self
				.queries
				.iter()
				.map(|q| super::types::LanguageQueryEntry {
					kind: ctx.intern(&q.kind),
					text: ctx.intern(&q.text),
				})
				.collect::<Vec<_>>()
				.into(),
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
				viewport_repair: l.viewport_repair.as_ref().map(|r| ViewportRepairPayload {
					enabled: r.enabled,
					max_scan_bytes: r.max_scan_bytes,
					prefer_real_closer: r.prefer_real_closer,
					max_forward_search_bytes: r.max_forward_search_bytes,
					rules: r
						.rules
						.iter()
						.map(|rule| match rule {
							super::spec::ViewportRepairRuleSpec::BlockComment { open, close, nestable } => ViewportRepairRulePayload::BlockComment {
								open: open.clone(),
								close: close.clone(),
								nestable: *nestable,
							},
							super::spec::ViewportRepairRuleSpec::String { quote, escape } => ViewportRepairRulePayload::String {
								quote: quote.clone(),
								escape: escape.clone(),
							},
							super::spec::ViewportRepairRuleSpec::LineComment { start } => ViewportRepairRulePayload::LineComment { start: start.clone() },
						})
						.collect(),
				}),
				queries: l
					.queries
					.iter()
					.map(|q| LanguageQueryPayload {
						kind: q.kind.clone(),
						text: q.text.clone(),
					})
					.collect(),
			},
		})
		.collect()
}
