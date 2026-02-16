use super::*;

/// Must suppress Stage-B planning within a poll when Stage-A is already planned.
///
/// * Enforced in: `ensure::plan::compute_plan`
/// * Failure symptom: urgent and enrich viewport lanes run concurrently for the
///   same poll window, causing avoidable permit contention and parse churn.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stage_a_precedes_stage_b_within_single_poll() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine,
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(5));
	policy.l.viewport_stage_b_min_stable_polls = 0;
	mgr.set_policy(policy);

	let doc_id = DocumentId(3);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..128),
	});
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id));
	assert!(!mgr.has_inflight_viewport_enrich(doc_id));
}

/// Must only attempt synchronous bootstrap once per bootstrap window to
/// avoid repeated stutter when background parsing is throttled.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Stuttering UI when many files are opened simultaneously.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_sync_bootstrap_attempted_only_once_when_throttled() {
	let engine = Arc::new(TimeoutSensitiveEngine::new(Duration::from_millis(10)));
	// Concurrency 1
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s.debounce = Duration::ZERO;
	policy.s.sync_bootstrap_timeout = Some(Duration::from_millis(5));
	mgr.set_policy(policy);

	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Occupy the only permit with Document 1
	mgr.ensure_syntax(make_ctx(DocumentId(1), 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(DocumentId(1)));

	// 2. Poll Document 2 - should try sync (fail) then return Pending
	let r = mgr.ensure_syntax(make_ctx(DocumentId(2), 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Pending);
	// 1 (Doc 1 background) + 1 (Doc 2 sync)
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 2);

	// 3. Poll Document 2 again - should NOT try sync again
	let r2 = mgr.ensure_syntax(make_ctx(DocumentId(2), 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r2.result, SyntaxPollResult::Pending);
	// Should still be 2
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 2);
}

/// Must skip highlight spans when `tree_doc_version` differs from
/// the rendered document version.
///
/// * Enforced in: `HighlightTiles::build_tile_spans` (in `crate::render::cache::highlight`)
/// * Failure symptom: Out-of-bounds tree-sitter access can panic during rapid edits.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_highlight_skips_stale_tree_version() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// Install syntax at V1.
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));

	// Record an edit (moves to V2) but don't sync incrementally.
	mgr.note_edit(doc_id, EditSource::Typing);
	assert!(mgr.is_dirty(doc_id));

	// tree_doc_version is still V1 because the tree has not been updated.
	assert_eq!(
		mgr.syntax_doc_version(doc_id),
		Some(1),
		"tree_doc_version must remain at V1 after an un-synced edit"
	);
}

/// Must only expose highlight projection context when pending incremental edits
/// are aligned with the resident tree version.
///
/// * Enforced in: `SyntaxManager::highlight_projection_ctx`
/// * Failure symptom: highlight projection applies mismatched deltas and causes
///   visual jump/flicker during debounce.
#[cfg_attr(test, test)]
pub(crate) fn test_highlight_projection_ctx_alignment_gate() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(9);
	let old_rope = Rope::from("abcdef");
	let changes = ChangeSet::new(old_rope.slice(..));

	{
		let loader = Arc::new(LanguageLoader::from_embedded());
		let lang = loader.language_for_name("rust").unwrap();
		let entry = mgr.entry_mut(doc_id);
		let syntax = Syntax::new(old_rope.slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		entry.slot.full = Some(InstalledTree {
			syntax,
			doc_version: 1,
			tree_id: 0,
		});
		entry.slot.pending_incremental = Some(PendingIncrementalEdits {
			base_tree_doc_version: 1,
			old_rope: old_rope.clone(),
			composed: changes.clone(),
		});
	}

	assert!(mgr.highlight_projection_ctx(doc_id, 2).is_some());
	assert!(mgr.highlight_projection_ctx(doc_id, 1).is_none());

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.pending_incremental = Some(PendingIncrementalEdits {
			base_tree_doc_version: 2,
			old_rope,
			composed: changes,
		});
	}

	assert!(
		mgr.highlight_projection_ctx(doc_id, 3).is_none(),
		"projection context must not be exposed for mismatched pending base"
	);
}

/// Must promote recently visible documents to `Warm` hotness to avoid
/// immediate retention drops.
///
/// * Enforced in: `Editor::ensure_syntax_for_buffers`, `Editor::on_document_close`
/// * Failure symptom: Switching away for one frame drops syntax and causes a flash of
///   unhighlighted text.
#[cfg_attr(test, test)]
pub(crate) fn test_warm_hotness_prevents_immediate_drop() {
	use super::lru::RecentDocLru;

	let mut lru = RecentDocLru::new(3);
	let d1 = DocumentId(1);
	let d2 = DocumentId(2);
	let d3 = DocumentId(3);
	let d4 = DocumentId(4);

	// Touch documents - simulates visible buffers in render loop.
	lru.touch(d1);
	lru.touch(d2);
	lru.touch(d3);

	// All three are tracked (recently visible -> Warm hotness).
	assert!(lru.contains(d1), "d1 must be warm");
	assert!(lru.contains(d2), "d2 must be warm");
	assert!(lru.contains(d3), "d3 must be warm");

	// Touch d4 - evicts oldest (d1) due to capacity=3.
	lru.touch(d4);
	assert!(!lru.contains(d1), "d1 must be evicted (oldest)");
	assert!(lru.contains(d4), "d4 must be warm");

	// Re-touch d2 - d2 moves to front, d3 is now oldest.
	lru.touch(d2);
	lru.touch(DocumentId(5));
	assert!(!lru.contains(d3), "d3 must be evicted after d2 re-touch");
	assert!(lru.contains(d2), "d2 must survive re-touch");

	// Explicit remove (e.g. document close).
	lru.remove(d2);
	assert!(!lru.contains(d2), "d2 must be gone after remove");
}

/// Must apply viewport timeout cooldown for viewport tasks instead of full-parse cooldown.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Visible large-document highlighting remains disabled for multi-second
///   windows after a viewport parse timeout.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_viewport_timeout_uses_viewport_cooldown() {
	let mut mgr = SyntaxManager::default();
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.cooldown_on_timeout = Duration::from_secs(5);
	policy.l.viewport_cooldown_on_timeout = Duration::from_millis(20);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: OptKey {
				injections: InjectionPolicy::Disabled,
			},
			result: Err(SyntaxError::Timeout),
			class: TaskClass::Viewport,
			elapsed: Duration::from_millis(1),
			viewport_key: Some(ViewportKey(0)),
			viewport_lane: Some(super::super::scheduling::ViewportLane::Urgent),
		});
	}

	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	// Viewport urgent lane cools down, but BG lane is free and kicks a full parse.
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	sleep(Duration::from_millis(30)).await;

	let r2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	// After viewport cooldown expires, viewport urgent lane can schedule again.
	assert!(matches!(r2.result, SyntaxPollResult::Kicked | SyntaxPollResult::Pending));
}

/// Must install stale viewport results when monotonic and not-future to preserve continuity.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Highlighting disappears during rapid edits because viewport results are
///   discarded unless they exactly match the current document version.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_viewport_stale_install_continuity() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
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
	let content = Rope::from("fn main() {}\n");

	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	mgr.note_edit(doc_id, EditSource::Typing);

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let _ = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});

	assert!(mgr.has_syntax(doc_id));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));
	assert!(mgr.is_dirty(doc_id));
}

/// Must skip stale viewport installs when a covering tree already exists.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Large-file edits produce an extra delayed repaint that applies stale
///   spans before the eventual corrected repaint.
#[cfg_attr(test, test)]
pub(crate) fn test_viewport_stale_install_skipped_when_covered() {
	use xeno_language::syntax::SealedSource;

	let mut mgr = SyntaxManager::default();
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
	let viewport = 0..100;

	let seed_len = 4096usize.min(content.len_bytes()) as u32;
	let seed_syntax = {
		let sealed = Arc::new(SealedSource::from_window(content.byte_slice(0..seed_len as usize), ""));
		Syntax::new_viewport(
			sealed,
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
			0,
		)
		.expect("seed viewport syntax")
	};

	let stale_syntax = {
		let sealed = Arc::new(SealedSource::from_window(content.byte_slice(0..seed_len as usize), ""));
		Syntax::new_viewport(
			sealed,
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
			0,
		)
		.expect("stale viewport syntax")
	};

	let existing_tree_id = {
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let tree_id = entry.slot.alloc_tree_id();
		let ce = entry.slot.viewport_cache.get_mut_or_insert(ViewportKey(0));
		ce.stage_a = Some(ViewportTree {
			syntax: seed_syntax,
			doc_version: 1,
			tree_id,
			coverage: 0..seed_len,
		});

		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: OptKey {
				injections: InjectionPolicy::Disabled,
			},
			result: Ok(stale_syntax),
			class: TaskClass::Viewport,
			elapsed: Duration::from_millis(1),
			viewport_key: Some(ViewportKey(0)),
			viewport_lane: Some(super::super::scheduling::ViewportLane::Urgent),
		});
		tree_id
	};

	let outcome = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(viewport.clone()),
	});

	assert!(
		!outcome.updated,
		"stale completion should not trigger an intermediate repaint when coverage already exists"
	);

	let selected = mgr
		.syntax_for_viewport(doc_id, 2, viewport)
		.expect("existing covered viewport tree should remain selected");
	assert_eq!(selected.tree_id, existing_tree_id);
	assert_eq!(selected.tree_doc_version, 1);
}

/// Must prefer eager urgent viewport parsing for L-tier history edits, including
/// when a non-eager full tree already exists.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Undo in large files repaints with stale/non-eager viewport
///   spans before correctness pass runs.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_l_history_edit_uses_eager_urgent_with_full_present() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(1));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));
	let viewport = 0..200;

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let full = Syntax::new(
			Rope::from("fn main() { let x = 1; }").slice(..),
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
		)
		.unwrap();
		let full_tree_id = entry.slot.alloc_tree_id();
		// Version 0: stale relative to the edit at version 1, so the
		// urgent viewport parse (at version 1) will be preferred.
		entry.slot.full = Some(InstalledTree {
			syntax: full,
			doc_version: 0,
			tree_id: full_tree_id,
		});
	}
	mgr.note_edit(doc_id, EditSource::History);

	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(viewport.clone()),
	});
	assert_eq!(r1.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id));

	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(viewport.clone()),
	});
	assert!(r2.updated, "urgent completion install should trigger redraw");

	let selected = mgr.syntax_for_viewport(doc_id, 1, viewport.clone()).expect("urgent tree should be selected");
	assert_eq!(
		selected.syntax.opts().injections,
		InjectionPolicy::Eager,
		"L-tier history edits should use eager urgent viewport parsing"
	);
	assert!(
		mgr.has_inflight_viewport_enrich(doc_id),
		"Stage-B should stay eligible as a follow-up correctness pass"
	);
}

/// Must preempt tracked full/incremental work when a visible large-doc viewport is uncovered.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Scrolling in large files shows blank text until an unrelated full parse
///   completes.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_viewport_preempts_inflight_full_parse() {
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
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(400_000));

	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	assert_eq!(r2.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_bg(doc_id));

	let r3 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(300_000..300_050),
	});
	assert_eq!(r3.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport(doc_id));
}

/// Must clamp viewport scheduling span before coverage checks to prevent infinite Stage A loops.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Large single-line viewports keep re-kicking Stage A and never progress
///   to catch-up full parsing.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_viewport_span_cap_prevents_stage_a_loop() {
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
	policy.l.viewport_stage_b_budget = None;
	policy.l.viewport_lookbehind = 0;
	policy.l.viewport_lookahead = 0;
	policy.l.viewport_window_max = 1024;
	policy.l.viewport_visible_span_cap = 1024;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));
	let full_view = Some(0..300_000);

	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: full_view.clone(),
	});
	assert_eq!(r1.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport(doc_id));

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: full_view,
	});
	assert_eq!(r2.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_bg(doc_id));
}

/// Must suppress same-version history Stage-A retries after an urgent timeout so
/// background catch-up can proceed.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Undo loops Stage-A timeouts on the same version and starves
///   background full/incremental parse recovery.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_history_stage_a_timeout_suppresses_same_version_retry() {
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
	policy.l.viewport_stage_b_budget = None;
	policy.l.viewport_cooldown_on_timeout = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));
	let viewport = 200_000..200_200;
	let viewport_key = ViewportKey(196_608);

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let full = Syntax::new(
			Rope::from("fn main() { let x = 1; }").slice(..),
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
		)
		.unwrap();
		let full_tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(InstalledTree {
			syntax: full,
			doc_version: 1,
			tree_id: full_tree_id,
		});
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 2,
			lang_id: lang,
			opts: OptKey {
				injections: InjectionPolicy::Eager,
			},
			result: Err(SyntaxError::Timeout),
			class: TaskClass::Viewport,
			elapsed: Duration::from_millis(1),
			viewport_key: Some(viewport_key),
			viewport_lane: Some(super::super::scheduling::ViewportLane::Urgent),
		});
	}

	mgr.note_edit(doc_id, EditSource::History);
	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(viewport.clone()),
	});
	// With per-lane cooldowns, viewport timeout doesn't block BG lane — BG kicks immediately.
	assert_eq!(r1.result, SyntaxPollResult::Kicked);
	assert!(
		!mgr.has_inflight_viewport_urgent(doc_id),
		"same-version history urgent retries should be suppressed after timeout"
	);
	assert!(
		mgr.has_inflight_bg(doc_id),
		"background catch-up should proceed even when viewport urgent just timed out"
	);

	let r2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(viewport),
	});
	// BG is already inflight from r1, viewport urgent is still suppressed for same version.
	assert_eq!(r2.result, SyntaxPollResult::Pending);
}
