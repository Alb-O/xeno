mod derive;
mod incremental;
mod install;
mod parse_mode;
mod planning;

use xeno_language::SyntaxError;

use super::install::{InstallDecision, decide_install, install_completions};
use super::*;

/// Helper: builds a dummy CompletedSyntaxTask for decision tests (result is Err so no Syntax needed).
fn dummy_completed(
	doc_version: u64,
	lang_id: xeno_language::LanguageId,
	class: TaskClass,
	injections: InjectionPolicy,
	viewport_key: Option<ViewportKey>,
	viewport_lane: Option<scheduling::ViewportLane>,
) -> CompletedSyntaxTask {
	CompletedSyntaxTask {
		doc_version,
		lang_id,
		opts: OptKey { injections },
		result: Err(SyntaxError::Timeout),
		class,
		elapsed: Duration::ZERO,
		viewport_key,
		viewport_lane,
	}
}

fn make_derive_ctx<'a>(
	content: &'a Rope,
	loader: &'a Arc<LanguageLoader>,
	viewport: Option<std::ops::Range<u32>>,
	hotness: SyntaxHotness,
) -> EnsureSyntaxContext<'a> {
	EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: None,
		content,
		hotness,
		loader,
		viewport,
	}
}
