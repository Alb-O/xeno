use super::*;

#[tokio::test]
async fn test_stage_b_requires_stable_polls() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 3;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Bootstrap with a full tree so Stage-B has something to enrich.
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions {
					injections: InjectionPolicy::Disabled,
					..Default::default()
				},
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Poll 1 — should NOT kick Stage-B (1 < 3)
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "poll 1: not enough stable polls");

	// Poll 2 — still not enough (2 < 3)
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "poll 2: not enough stable polls");

	// Poll 3 — now it should fire (3 >= 3)
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert!(mgr.has_inflight_viewport_enrich(doc_id), "poll 3: should kick Stage-B");
}

/// Stage-B stability gating must track the covering enrichment key, not just
/// the computed viewport anchor key.
#[tokio::test]
async fn test_stage_b_stability_uses_covering_key_across_stride_boundary() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_window_max = 65_536;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 2;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed a full tree and a wide stage_a cache entry at ViewportKey(0) that
	// covers both viewports, even though their computed anchor keys differ.
	{
		let entry = mgr.entry_mut(doc_id);
		let syntax = Syntax::new(
			content.slice(..),
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
		)
		.unwrap();
		let full_tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(syntax.clone());
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = full_tree_id;
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;

		let stage_a_tree_id = entry.slot.alloc_tree_id();
		let cache_entry = entry.slot.viewport_cache.get_mut_or_insert(ViewportKey(0));
		cache_entry.stage_a = Some(ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: stage_a_tree_id,
			coverage: 0..200_000,
		});
	}

	// Poll 1: stable polls = 1, should not kick Stage-B yet.
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(65_000..65_100),
	});
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "poll 1: not enough stable polls");

	// Poll 2: viewport crosses stride boundary, but covering key remains 0, so
	// Stage-B should now kick at stable polls = 2.
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(70_000..70_100),
	});
	assert!(
		mgr.has_inflight_viewport_enrich(doc_id),
		"poll 2: should kick Stage-B when covering key is stable"
	);
}

/// When the viewport key flips every poll (fast scrolling), the stable poll
/// counter resets and Stage-B never fires.
#[tokio::test]
async fn test_stage_b_does_not_kick_on_scroll_key_flip() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 2;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Bootstrap with a full tree.
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions {
					injections: InjectionPolicy::Disabled,
					..Default::default()
				},
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Alternate viewport positions so the key keeps changing
	let viewports = [0..100, 200_000..200_100, 0..100, 200_000..200_100];
	for vp in &viewports {
		mgr.ensure_syntax(EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
			viewport: Some(vp.clone()),
		});
		assert!(
			!mgr.has_inflight_viewport_enrich(doc_id),
			"Stage-B should not kick during fast scrolling (viewport key keeps flipping)"
		);
	}
}

/// When Stage-B enrichment is desired but deferred (stability gate not yet
/// met), the poll result must be Pending, not Ready.
#[tokio::test]
async fn test_stage_b_deferral_returns_pending_not_throttled() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 3;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed full tree so dirty=false and full exists
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions {
					injections: InjectionPolicy::Disabled,
					..Default::default()
				},
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Poll once — stability gate not met (1 < 3), should return Pending
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_eq!(r.result, SyntaxPollResult::Pending, "enrich desired but deferred → Pending");
	assert!(!mgr.has_inflight_viewport_enrich(doc_id));
}

/// When the document is fully parsed at the current version with no pending
/// work, the poll result must be Ready.
#[tokio::test]
async fn test_no_work_returns_ready() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(SyntaxManagerCfg::default(), engine.clone());
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// Seed full tree at exact version, not dirty
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions {
					injections: InjectionPolicy::Eager,
					..Default::default()
				},
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
		entry.slot.last_opts_key = Some(super::types::OptKey {
			injections: InjectionPolicy::Eager,
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Ready, "fully parsed, no work → Ready");
}
