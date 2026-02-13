use super::*;

#[tokio::test]
async fn test_stage_b_is_tracked_per_key() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine.clone(),
	);

	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_millis(500));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() { let x = 1; }");

	// Install a full tree so Stage-A is not needed
	let full_tree = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions {
			injections: InjectionPolicy::Disabled,
			..Default::default()
		},
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(full_tree);
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tid;
		entry.slot.dirty = false;
		entry.slot.language_id = Some(lang);
		entry.slot.last_opts_key = Some(OptKey {
			injections: InjectionPolicy::Disabled,
		});
	}

	// First viewport → should kick Stage-B enrichment
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..20),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);

	// Complete the task
	engine.proceed();
	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	mgr.drain_finished_inflight();

	// Same viewport key again → should NOT kick (attempted_b_for is set)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..20),
	});
	// Should be Ready since we have full tree + enrichment was attempted
	assert_ne!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_viewport_kicks_while_bg_running() {
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
	// Force tier L so viewport scheduling activates
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// First ensure: should kick viewport (Stage-A) since no tree exists
	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_eq!(r1.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport(doc_id), "viewport lane should be active");
	assert!(mgr.has_inflight_bg(doc_id), "bg lane should be active");

	// Bg uses engine.parse (blocks on notify); viewport runs directly via Syntax::new_viewport.
	let mut iters = 0;
	while engine.parse_count.load(Ordering::SeqCst) < 1 && iters < 200 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1, "bg should have entered engine");

	// Unblock the bg task
	engine.proceed();
	super::invariants::wait_for_finish(&mgr).await;
	assert!(mgr.drain_finished_inflight());
}

#[tokio::test]
async fn test_stage_a_kicks_when_partial_overlap_only() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	// Seed a viewport tree that overlaps but doesn't fully cover [1000..2000]
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let tree_id = entry.slot.alloc_tree_id();
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		let key = super::types::ViewportKey(0);
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id,
			coverage: 500..1500, // overlaps [1000..2000] but doesn't cover it
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(1000..2000),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id), "Stage-A should kick for partial overlap");
}

#[tokio::test]
async fn test_stage_a_can_kick_while_enrich_lane_active() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	// Pre-seed: enrich lane active, no full tree, viewport uncovered
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.active_viewport_enrich = Some(super::types::TaskId(123));
		entry.sched.active_viewport_enrich_detached = false;
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id), "Stage-A should kick even with enrich active");
}

#[tokio::test]
async fn test_l_retention_drops_full_keeps_viewport_cache() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.l.retention_hidden_viewport = RetentionPolicy::Keep;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed full tree + viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

		let tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(syntax.clone());
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tree_id;

		let vp_tree_id = entry.slot.alloc_tree_id();
		let key = super::types::ViewportKey(0);
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: vp_tree_id,
			coverage: 0..50000,
		});
	}

	// Ensure with Cold → retention should drop full but keep viewport
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Disabled);

	let entry = mgr.entry_mut(doc_id);
	assert!(entry.slot.full.is_none(), "full tree should be dropped");
	assert!(entry.slot.viewport_cache.has_any(), "viewport cache should be kept");
}

#[tokio::test]
async fn test_l_retention_drops_viewport_when_configured() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.l.retention_hidden_viewport = RetentionPolicy::DropWhenHidden;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		let vp_tree_id = entry.slot.alloc_tree_id();
		let key = super::types::ViewportKey(0);
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: vp_tree_id,
			coverage: 0..50000,
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Disabled);

	let entry = mgr.entry_mut(doc_id);
	assert!(!entry.slot.viewport_cache.has_any(), "viewport cache should be dropped");
}

#[tokio::test]
async fn test_stage_b_timeout_sets_per_key_cooldown_and_allows_retry() {
	use xeno_language::syntax::SyntaxError;

	use super::scheduling::{CompletedSyntaxTask, ViewportLane};

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
	policy.l.viewport_cooldown_on_timeout = Duration::ZERO; // instant retry
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed: full tree + viewport cache with stage_a
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(
			Rope::from("fn main() {}").slice(..),
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
		)
		.unwrap();
		let tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(syntax.clone());
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tree_id;

		let key = super::types::ViewportKey(0);
		let vp_tree_id = entry.slot.alloc_tree_id();
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: vp_tree_id,
			coverage: 0..50000,
		});

		// Inject Stage-B timeout completion
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: super::types::OptKey {
				injections: InjectionPolicy::Eager,
			},
			result: Err(SyntaxError::Timeout),
			class: super::tasks::TaskClass::Viewport,
			injections: InjectionPolicy::Eager,
			elapsed: Duration::from_millis(100),
			viewport_key: Some(key),
			viewport_lane: Some(ViewportLane::Enrich),
		});
	}

	// First ensure: should process the timeout but NOT return CoolingDown
	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_ne!(r1.result, SyntaxPollResult::CoolingDown, "Stage-B timeout should not cause global cooldown");
	// The same ensure call processed the timeout (clearing latch) and immediately re-kicked Stage-B
	assert!(mgr.has_inflight_viewport_enrich(doc_id), "Stage-B should retry immediately with zero cooldown");
}

#[tokio::test]
async fn test_stage_b_failure_does_not_block_stage_a() {
	use xeno_language::syntax::SyntaxError;

	use super::scheduling::{CompletedSyntaxTask, ViewportLane};

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
	policy.l.viewport_cooldown_on_error = Duration::from_secs(9999);
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed: no full tree, inject Stage-B error into completion queue
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: super::types::OptKey {
				injections: InjectionPolicy::Eager,
			},
			result: Err(SyntaxError::Parse("test error".to_string())),
			class: super::tasks::TaskClass::Viewport,
			injections: InjectionPolicy::Eager,
			elapsed: Duration::from_millis(50),
			viewport_key: Some(super::types::ViewportKey(0)),
			viewport_lane: Some(ViewportLane::Enrich),
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_ne!(r.result, SyntaxPollResult::CoolingDown, "Stage-B error should not cause global cooldown");
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id), "Stage-A should still kick despite Stage-B error");
}
