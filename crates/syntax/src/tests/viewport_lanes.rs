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
		entry.slot.full = Some(InstalledTree {
			syntax: full_tree,
			doc_version: 1,
			tree_id: tid,
		});
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
		entry.sched.lanes.viewport_enrich.active = Some(super::types::TaskId(123));
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

	// Seed full tree + viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.last_tier = Some(super::policy::SyntaxTier::L);
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

		let tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(InstalledTree {
			syntax: syntax.clone(),
			doc_version: 1,
			tree_id,
		});

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

	// Sweep retention as Cold → should drop full but keep viewport
	mgr.sweep_retention(Instant::now(), |_| SyntaxHotness::Cold);

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

	// Seed viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.last_tier = Some(super::policy::SyntaxTier::L);
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

	// Sweep retention as Cold → should drop viewport
	mgr.sweep_retention(Instant::now(), |_| SyntaxHotness::Cold);

	let entry = mgr.entry_mut(doc_id);
	assert!(!entry.slot.viewport_cache.has_any(), "viewport cache should be dropped");
}

#[tokio::test]
async fn test_stage_b_timeout_sets_per_key_cooldown_and_allows_retry() {
	use xeno_language::SyntaxError;

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
		entry.slot.full = Some(InstalledTree {
			syntax: syntax.clone(),
			doc_version: 1,
			tree_id,
		});

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
	use xeno_language::SyntaxError;

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

/// Stage-B completion must install with eager injections in the viewport cache.
///
/// Regression test: opts_key on viewport TaskSpecs must match the actual injection
/// policy used, not the tier default. Otherwise Stage-B results fail the `opts_ok`
/// check and are silently discarded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_stage_b_completion_installs_eager_tree() {
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
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed a full tree so Stage-A is not needed and Stage-B can kick.
	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(InstalledTree {
			syntax: Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions {
					injections: InjectionPolicy::Disabled,
					..Default::default()
				},
			)
			.unwrap(),
			doc_version: 1,
			tree_id: tid,
		});
		entry.slot.dirty = false;
		entry.slot.language_id = Some(lang);
		entry.slot.last_opts_key = Some(OptKey {
			injections: InjectionPolicy::Disabled,
		});
	}

	// First ensure → should kick Stage-B enrichment.
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
	assert!(mgr.has_inflight_viewport_enrich(doc_id), "Stage-B should have kicked");

	// Complete the task via mock engine.
	engine.proceed();
	super::invariants::wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// Second ensure → should install the Stage-B result.
	let _r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});

	// The Stage-B tree should now be installed in the viewport cache.
	let entry = mgr.entry_mut(doc_id);
	let vp = 0u32..100u32;
	let covering_key = entry.slot.viewport_cache.covering_key(&vp);
	assert!(covering_key.is_some(), "should have a covering viewport key");
	let ce = entry.slot.viewport_cache.map.get(&covering_key.unwrap()).unwrap();
	assert!(ce.stage_b.is_some(), "Stage-B tree should be installed after completion");
	assert_eq!(
		ce.stage_b.as_ref().unwrap().syntax.opts().injections,
		InjectionPolicy::Eager,
		"Stage-B tree should have eager injections"
	);
}

/// History-urgent Stage-A must install with eager injections.
///
/// Regression test: when last_edit_source is History and tier is L, Stage-A
/// uses eager injections. The opts_key must match so the install check passes.
#[tokio::test]
async fn test_history_urgent_stage_a_installs_eager_tree() {
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
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Set up history edit source so Stage-A uses eager injections.
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.last_edit_source = EditSource::History;
	}

	// Kick Stage-A (no tree → viewport uncovered → kicks).
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
	assert!(mgr.has_inflight_viewport_urgent(doc_id));

	// Complete the viewport task.
	// Viewport tasks run via Syntax::new_viewport directly (not through engine.parse),
	// so they complete immediately without needing engine.proceed().
	super::invariants::wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// Install the completion.
	let _r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});

	// The viewport cache should have an installed Stage-A tree with eager injections.
	let entry = mgr.entry_mut(doc_id);
	let vp = 0u32..100u32;
	let covering_key = entry.slot.viewport_cache.covering_key(&vp);
	assert!(covering_key.is_some(), "should have a covering viewport key");
	let ce = entry.slot.viewport_cache.map.get(&covering_key.unwrap()).unwrap();
	assert!(ce.stage_a.is_some(), "Stage-A tree should be installed after history-urgent completion");
	assert_eq!(
		ce.stage_a.as_ref().unwrap().syntax.opts().injections,
		InjectionPolicy::Eager,
		"History-urgent Stage-A tree should have eager injections"
	);
}
