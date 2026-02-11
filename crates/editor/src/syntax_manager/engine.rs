use xeno_language::syntax::{Syntax, SyntaxError, SyntaxOptions};
use xeno_language::{LanguageId, LanguageLoader};
use xeno_primitives::ChangeSet;

/// Abstract engine for parsing syntax (for test mockability).
pub trait SyntaxEngine: Send + Sync {
	fn parse(&self, content: ropey::RopeSlice<'_>, lang: LanguageId, loader: &LanguageLoader, opts: SyntaxOptions) -> Result<Syntax, SyntaxError>;

	/// Incrementally updates an existing syntax tree via a composed changeset.
	///
	/// The default implementation discards the old tree and falls back to a
	/// full reparse, allowing mock engines to remain simple.
	fn update_incremental(
		&self,
		_syntax: Syntax,
		_old_source: ropey::RopeSlice<'_>,
		new_source: ropey::RopeSlice<'_>,
		_changeset: &ChangeSet,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		self.parse(new_source, lang, loader, opts)
	}
}

pub(super) struct RealSyntaxEngine;
impl SyntaxEngine for RealSyntaxEngine {
	fn parse(&self, content: ropey::RopeSlice<'_>, lang: LanguageId, loader: &LanguageLoader, opts: SyntaxOptions) -> Result<Syntax, SyntaxError> {
		Syntax::new(content, lang, loader, opts)
	}

	fn update_incremental(
		&self,
		mut syntax: Syntax,
		old_source: ropey::RopeSlice<'_>,
		new_source: ropey::RopeSlice<'_>,
		changeset: &ChangeSet,
		_lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		syntax
			.update_from_changeset(old_source, new_source, changeset, loader, opts)
			.map(|()| syntax)
			.or_else(|e| {
				tracing::warn!(error = %e, "Incremental parse failed, falling back to full reparse");
				Syntax::new(new_source, _lang, loader, opts)
			})
	}
}
