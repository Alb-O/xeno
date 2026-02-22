use super::*;

/// S-tier visible document gets sync bootstrap on first poll: full tree installed immediately.
#[test]
fn parse_mode_s_tier_sync_bootstrap() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	// Small content → S tier
	let content = Rope::from("fn main() {}");
	let doc_id = DocumentId(1);

	let mut mgr = SyntaxManager::new(Default::default());
	let outcome = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});

	assert_eq!(outcome.result, SyntaxPollResult::Ready, "S-tier first poll must return Ready (sync bootstrap)");
	assert!(outcome.updated, "S-tier first poll must report updated");

	let state = mgr.debug_doc_state(doc_id).unwrap();
	assert!(state.sync_bootstrap_attempted, "sync bootstrap must be attempted for S-tier");
	assert_eq!(state.full_doc_version, Some(1), "full tree must be installed at doc version 1");
	assert!(!state.dirty, "dirty must be cleared after bootstrap install");
	assert!(!state.bg_inflight, "no BG task needed after successful bootstrap");
}

/// L-tier visible document skips sync bootstrap, schedules background parse.
#[tokio::test]
async fn parse_mode_l_tier_bg_parse() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	// Content > 1MB → L tier
	let content = Rope::from("x".repeat(2_000_000));
	let doc_id = DocumentId(2);

	let mut mgr = SyntaxManager::new(Default::default());
	let outcome = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..200),
	});

	// L-tier has no sync_bootstrap_timeout → should skip bootstrap and schedule work
	assert_ne!(outcome.result, SyntaxPollResult::Ready, "L-tier first poll must not return Ready");

	let state = mgr.debug_doc_state(doc_id).unwrap();
	assert!(!state.sync_bootstrap_attempted, "L-tier has no sync_bootstrap_timeout, flag stays false");
	assert!(state.full_doc_version.is_none(), "no full tree installed without sync bootstrap");
	// Should have kicked viewport or BG task
	assert!(
		state.bg_inflight || state.viewport_urgent_inflight,
		"L-tier must schedule background or viewport task"
	);
}

/// M-tier visible document gets sync bootstrap same as S-tier.
#[test]
fn parse_mode_m_tier_sync_bootstrap() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	// Content between 256KB and 1MB → M tier
	let content = Rope::from("x".repeat(500_000));
	let doc_id = DocumentId(3);

	let mut mgr = SyntaxManager::new(Default::default());
	let outcome = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});

	// M-tier has sync_bootstrap_timeout (3ms). For 500KB of "x" repeated, the parse
	// may succeed or timeout. We verify the bootstrap was at least attempted.
	let state = mgr.debug_doc_state(doc_id).unwrap();
	assert!(state.sync_bootstrap_attempted, "sync bootstrap must be attempted for M-tier");
	// If bootstrap succeeded, full tree installed and ready
	if outcome.result == SyntaxPollResult::Ready {
		assert_eq!(state.full_doc_version, Some(1));
		assert!(!state.dirty);
	}
}
