use xeno_language::{SyntaxError, SyntaxOptions};

use super::install::{InstallDecision, decide_install, install_completions};
use super::*;

#[test]
fn test_compute_viewport_key_aligns_to_half_window_stride() {
	let key = compute_viewport_key(70_000, 131_072);
	assert_eq!(key, ViewportKey(65_536));
}

#[test]
fn test_compute_viewport_key_respects_min_stride_floor() {
	let key = compute_viewport_key(9_000, 4_096);
	assert_eq!(key, ViewportKey(8_192));
}

/// When Stage-A is planned, Stage-B must not be planned (avoids concurrent
/// viewport parsing contention).
#[test]
fn test_plan_stage_a_suppresses_stage_b() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut entry = DocEntry::new(Instant::now());
	entry.slot.language_id = Some(lang);
	// No full tree → Stage-A will be planned for viewport uncovered.

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
		p.l.viewport_stage_b_min_stable_polls = 0;
		p
	};

	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	};

	let d = derive(&ctx, &policy);
	let lang_ctx = d.clone().into_lang(lang);
	let g = GateState {
		viewport_stable_polls: 10, // well above min
		viewport_uncovered: true,
	};

	let plan = compute_plan(&entry, Instant::now(), &lang_ctx, &g, &SyntaxMetrics::new());
	assert!(plan.stage_a.is_some(), "Stage-A should be planned (viewport uncovered)");
	assert!(plan.stage_b.is_none(), "Stage-B must not be planned when Stage-A is planned");
}

/// BG can be planned independently of Stage-A.
#[test]
fn test_plan_bg_independent_of_stage_a() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut entry = DocEntry::new(Instant::now());
	entry.slot.language_id = Some(lang);
	entry.slot.dirty = true;
	// No full tree → both Stage-A and BG should plan.

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p.l.viewport_stage_b_budget = None;
		p
	};

	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	};

	let d = derive(&ctx, &policy);
	let lang_ctx = d.clone().into_lang(lang);
	let g = GateState::default();

	let plan = compute_plan(&entry, Instant::now(), &lang_ctx, &g, &SyntaxMetrics::new());
	assert!(plan.stage_a.is_some(), "Stage-A should be planned");
	assert!(plan.bg.is_some(), "BG should be planned independently of Stage-A");
}

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

/// Stale viewport completion must only install when no covering tree exists (continuity).
///
/// If the viewport is already covered by a full tree or viewport cache, stale results
/// are discarded to avoid installing outdated trees over better ones.
#[test]
fn test_stale_viewport_discards_when_covered() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p
	};

	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 5,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	};
	let d = derive(&ctx, &policy);

	// Case 1: entry has a full tree → stale viewport should discard.
	let mut entry = DocEntry::new(Instant::now());
	entry.slot.language_id = Some(lang);
	entry.slot.full = Some(InstalledTree {
		syntax: xeno_language::Syntax::new(
			content.slice(..),
			lang,
			&loader,
			SyntaxOptions {
				parse_timeout: Duration::from_secs(5),
				injections: InjectionPolicy::Disabled,
			},
		)
		.unwrap(),
		doc_version: 3,
		tree_id: 0,
	});

	let done = dummy_completed(
		3,
		lang,
		TaskClass::Viewport,
		d.cfg.viewport_injections,
		Some(ViewportKey(0)),
		Some(scheduling::ViewportLane::Urgent),
	);
	let decision = decide_install(&done, Instant::now(), &d, &entry);
	assert!(
		matches!(decision, InstallDecision::Discard),
		"stale viewport should discard when full tree covers: {decision:?}"
	);

	// Case 2: entry has NO full tree and NO viewport coverage → stale should install (continuity).
	let mut entry2 = DocEntry::new(Instant::now());
	entry2.slot.language_id = Some(lang);
	let done2 = dummy_completed(
		3,
		lang,
		TaskClass::Viewport,
		d.cfg.viewport_injections,
		Some(ViewportKey(0)),
		Some(scheduling::ViewportLane::Urgent),
	);
	let decision2 = decide_install(&done2, Instant::now(), &d, &entry2);
	assert!(
		matches!(decision2, InstallDecision::Install),
		"stale viewport should install when uncovered: {decision2:?}"
	);
}

/// Stale full-parse install must preserve projection alignment.
///
/// When a full tree already exists and `pending_incremental.base_tree_doc_version`
/// doesn't match the completed version, installing would break projection continuity.
#[test]
fn test_stale_full_discards_on_projection_mismatch() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	let policy = TieredSyntaxPolicy::test_default();
	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 5,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	};
	let d = derive(&ctx, &policy);

	// Full tree at version 2, pending_incremental base at version 2.
	// Completed task at version 3 → base mismatch (3 ≠ 2) → discard.
	let mut entry = DocEntry::new(Instant::now());
	entry.slot.language_id = Some(lang);
	entry.slot.dirty = true;
	entry.slot.full = Some(InstalledTree {
		syntax: xeno_language::Syntax::new(
			content.slice(..),
			lang,
			&loader,
			SyntaxOptions {
				parse_timeout: Duration::from_secs(5),
				injections: d.cfg.injections,
			},
		)
		.unwrap(),
		doc_version: 2,
		tree_id: 0,
	});
	entry.slot.pending_incremental = Some(PendingIncrementalEdits {
		base_tree_doc_version: 2,
		old_rope: content.clone(),
		composed: xeno_primitives::ChangeSet::new(content.slice(..)),
	});
	entry.sched.lanes.bg.requested_doc_version = 1;

	let done = dummy_completed(3, lang, TaskClass::Full, d.cfg.injections, None, None);
	let decision = decide_install(&done, Instant::now(), &d, &entry);
	assert!(
		matches!(decision, InstallDecision::Discard),
		"stale full with projection mismatch should discard: {decision:?}"
	);

	// Same setup but pending_incremental base matches done.doc_version → should install.
	entry.slot.pending_incremental = Some(PendingIncrementalEdits {
		base_tree_doc_version: 3,
		old_rope: content.clone(),
		composed: xeno_primitives::ChangeSet::new(content.slice(..)),
	});

	let done2 = dummy_completed(3, lang, TaskClass::Full, d.cfg.injections, None, None);
	let decision2 = decide_install(&done2, Instant::now(), &d, &entry);
	assert!(
		matches!(decision2, InstallDecision::Install),
		"stale full with matching projection should install: {decision2:?}"
	);
}

/// Documents with unprocessed completions must appear in `docs_with_completed()`.
#[test]
fn test_docs_with_completed_includes_queued() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();

	let mut mgr = SyntaxManager::new(Default::default());
	let doc_id = DocumentId(42);
	mgr.entry_mut(doc_id);
	assert!(mgr.docs_with_completed().next().is_none(), "no docs with completed initially");

	let done = dummy_completed(1, lang, TaskClass::Full, InjectionPolicy::Disabled, None, None);
	mgr.entry_mut(doc_id).sched.completed.push_back(done);
	let ids: Vec<_> = mgr.docs_with_completed().collect();
	assert_eq!(ids, vec![doc_id], "doc with queued completion should appear");
}

#[test]
fn test_install_completions_timeout_sets_urgent_lane_cooldown_without_update() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p
	};
	let now = Instant::now();
	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..128),
	};
	let d = derive(&ctx, &policy);
	let mut entry = DocEntry::new(now);
	entry.slot.language_id = Some(lang);
	entry.sched.completed.push_back(dummy_completed(
		1,
		lang,
		TaskClass::Viewport,
		d.cfg.viewport_injections,
		Some(ViewportKey(0)),
		Some(scheduling::ViewportLane::Urgent),
	));

	let mut metrics = SyntaxMetrics::new();
	let updated = install_completions(&mut entry, now, &d, &mut metrics);
	assert!(!updated, "timeout completion must not report syntax-tree updates");
	assert!(
		entry.sched.lanes.viewport_urgent.cooldown_until.is_some(),
		"urgent viewport timeout must enter viewport lane cooldown"
	);
}

#[test]
fn test_install_completions_stage_b_timeout_sets_key_cooldown() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p
	};
	let now = Instant::now();
	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..128),
	};
	let d = derive(&ctx, &policy);
	let mut entry = DocEntry::new(now);
	entry.slot.language_id = Some(lang);
	let key = ViewportKey(0);
	let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
	ce.attempted_b_for = Some(1);
	entry.sched.completed.push_back(dummy_completed(
		1,
		lang,
		TaskClass::Viewport,
		InjectionPolicy::Eager,
		Some(key),
		Some(scheduling::ViewportLane::Enrich),
	));

	let mut metrics = SyntaxMetrics::new();
	let updated = install_completions(&mut entry, now, &d, &mut metrics);
	assert!(!updated, "timeout completion must not report syntax-tree updates");
	let ce = entry.slot.viewport_cache.map.get(&key).expect("cache entry must exist");
	assert!(ce.stage_b_cooldown_until.is_some(), "Stage-B timeout must apply per-key viewport cooldown");
	assert_eq!(ce.attempted_b_for, None, "Stage-B timeout must clear attempted marker");
	assert!(
		entry.sched.lanes.viewport_enrich.cooldown_until.is_none(),
		"Stage-B cooldown must stay per-key and avoid lane-level cooldown"
	);
}

// --- Derive phase golden tests ---

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

#[test]
fn derive_viewport_clamps_to_doc_length() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello"); // 5 bytes
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, Some(0..1000), SyntaxHotness::Visible);
	let base = derive::derive(&ctx, &policy);

	assert!(base.viewport.is_some());
	let vp = base.viewport.unwrap();
	assert_eq!(vp.end, 5, "viewport end must clamp to doc length");
}

#[test]
fn derive_viewport_caps_span() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	// Create content larger than viewport_visible_span_cap (default 64KB for S).
	let content = Rope::from("x".repeat(200_000));
	let policy = TieredSyntaxPolicy::default();
	let cap = policy.s.viewport_visible_span_cap;

	let ctx = make_derive_ctx(&content, &loader, Some(0..200_000), SyntaxHotness::Visible);
	let base = derive::derive(&ctx, &policy);

	let vp = base.viewport.unwrap();
	assert_eq!(vp.end - vp.start, cap, "viewport span must be capped to visible_span_cap");
}

#[test]
fn derive_viewport_handles_reversed_range() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello world");
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, Some(8..3), SyntaxHotness::Visible);
	let base = derive::derive(&ctx, &policy);

	let vp = base.viewport.unwrap();
	assert!(vp.start <= vp.end, "reversed range must be normalized: start={}, end={}", vp.start, vp.end);
}

#[test]
fn derive_cold_hidden_disables_work() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello");
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, None, SyntaxHotness::Cold);
	let base = derive::derive(&ctx, &policy);

	assert!(base.work_disabled, "cold + parse_when_hidden=false must disable work");
}

#[test]
fn derive_warm_hidden_allows_work() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello");
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, None, SyntaxHotness::Warm);
	let base = derive::derive(&ctx, &policy);

	assert!(!base.work_disabled, "warm docs must allow work even with parse_when_hidden=false");
}

// --- Parse-mode selection golden tests ---

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
