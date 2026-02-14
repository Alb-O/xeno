use super::*;

/// Must clear `pending_incremental` on language change, syntax reset, and retention drop.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`, `SyntaxManager::reset_syntax`,
///   `apply_retention`
/// - Failure symptom: Stale changesets are applied against mismatched ropes, causing
///   bad edits or panics.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_language_switch_discards_old_parse() {
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
	let lang_rust = loader.language_for_name("rust").unwrap();
	let lang_py = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// Kick Rust parse
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_rust), &content, SyntaxHotness::Visible, &loader));

	// Switch to Python - invalidates Rust epoch, new task pending (wants work but permit held)
	let r = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_py), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Pending);

	// Rust result ready but discarded
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_py), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Kicked);

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_py), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Ready);
	assert!(mgr.has_syntax(doc_id));
}

/// Must tie background task permit lifetime to real thread execution.
///
/// - Enforced in: `TaskCollector::spawn`
/// - Failure symptom: Concurrency cap is violated under churn because permits are
///   released before CPU work ends.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_invalidate_does_not_release_permit_until_task_finishes() {
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

	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_rust = loader.language_for_name("rust").unwrap();
	let lang_py = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// Kick task for Doc 1
	mgr.ensure_syntax(make_ctx(DocumentId(1), 1, Some(lang_rust), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.pending_count(), 1);

	// Switch language -> invalidates epoch, but permit still held
	let r = mgr.ensure_syntax(make_ctx(DocumentId(1), 1, Some(lang_py), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Pending);

	// Allow first task to finish
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();

	// Now the new task can be kicked
	let r = mgr.ensure_syntax(make_ctx(DocumentId(1), 1, Some(lang_py), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Version monotonicity; a completed V5 parse must not clobber a
/// V7 tree that was installed via sync incremental updates.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_monotonic_version_guard() {
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
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish syntax at V5
	mgr.ensure_syntax(make_ctx(doc_id, 5, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 5, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(5));

	// Kick another parse at V5 (slow redundant one)
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(make_ctx(doc_id, 5, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(doc_id));

	// Advance to V7 via sync incremental updates
	mgr.note_edit_incremental(doc_id, 6, &content, &content, &ChangeSet::new(content.slice(..)), &loader, EditSource::Typing);
	mgr.note_edit_incremental(doc_id, 7, &content, &content, &ChangeSet::new(content.slice(..)), &loader, EditSource::Typing);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(7));

	// Complete the V5 parse
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// V5 must not clobber V7.
	mgr.ensure_syntax(make_ctx(doc_id, 10, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(7), "V5 should not clobber V7");
}

/// History edits bypass debounce.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_history_op_bypasses_debounce() {
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
	policy.s.debounce = Duration::from_secs(60);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish initial tree
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_syntax(doc_id));

	// Note edit (History)
	mgr.note_edit(doc_id, EditSource::History);
	assert!(mgr.is_dirty(doc_id));

	// Poll immediately - should NOT be debounced because History
	let r = mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Must preserve the resident full-tree version on history edits so undo can
/// project from the previous known-good tree while async lanes catch up.
///
/// - Enforced in: `SyntaxManager::note_edit_incremental`
/// - Failure symptom: Undo immediately replaces the syntax baseline with a
///   low-fidelity sync incremental result before viewport correction can run.
#[cfg_attr(test, test)]
pub(crate) fn test_history_incremental_preserves_resident_tree_version() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() { let x = 1; }\n");

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(content.slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		let tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(InstalledTree {
			syntax: syntax,
			doc_version: 1,
			tree_id: tree_id,
		});
	}

	let identity = ChangeSet::new(content.slice(..));
	mgr.note_edit_incremental(doc_id, 2, &content, &content, &identity, &loader, EditSource::History);

	let entry = mgr.entry_mut(doc_id);
	assert_eq!(entry.slot.full.as_ref().map(|t| t.doc_version), Some(1));
	assert!(entry.slot.pending_incremental.is_some());
	assert!(entry.slot.dirty);
	assert!(entry.sched.force_no_debounce);
}

/// Cold eviction and re-bootstrap; a document evicted due to Cold
/// hotness is re-bootstrapped immediately when it becomes visible again.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_eviction_reload() {
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
	policy.s.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.s.retention_hidden_viewport = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish syntax
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_syntax(doc_id));

	// Trigger eviction via sweep_retention (frame-level owner of retention)
	mgr.sweep_retention(Instant::now(), |_| SyntaxHotness::Cold);
	assert!(!mgr.has_syntax(doc_id));

	// Become visible again - should Kick bootstrap immediately
	let poll = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
}

/// Cold retention throttles work; Cold hotness plus DropWhenHidden
/// invalidates state and throttles new work until the permit is released.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_throttles_work() {
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
	policy.s.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.s.retention_hidden_viewport = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Start inflight parse
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(doc_id));

	// Drop hotness to Cold - should invalidate and return Disabled
	let poll = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Cold, &loader));
	assert_eq!(poll.result, SyntaxPollResult::Disabled);
	assert!(!mgr.has_pending(doc_id));

	// Permit still held - another doc is pending (wants work but can't run)
	let poll2 = mgr.ensure_syntax(make_ctx(DocumentId(2), 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(poll2.result, SyntaxPollResult::Pending);

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let poll3 = mgr.ensure_syntax(make_ctx(DocumentId(2), 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(poll3.result, SyntaxPollResult::Kicked);
}

/// Cold+DropWhenHidden invalidates the epoch, so returning to Visible re-kicks cleanly.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`, `SyntaxManager::sweep_retention`
/// - Failure symptom: Document stays Disabled or stale task result installs after visibility change.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_drop_then_visible_rekick() {
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
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.s.retention_hidden_viewport = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Visible -> kick task
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(doc_id));

	// Wait for background task to actually enter engine.parse
	let mut iters = 0;
	while engine.parse_count.load(Ordering::SeqCst) == 0 && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);

	// 2. Visible -> Cold with DropWhenHidden retention. Retention drops trees and
	// invalidates epoch, so the inflight task's result will be discarded.
	let r1 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Cold, &loader));
	assert_eq!(r1.result, SyntaxPollResult::Disabled);

	// 3. Cold -> Visible. Epoch was invalidated, so stale task result will be discarded.
	// Stale task still holds a permit, so BG can't spawn yet (Pending).
	let r2 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r2.result, SyntaxPollResult::Pending);

	// 4. Complete stale task, releasing the permit.
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// 5. Now a fresh task can be kicked.
	let r3 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r3.result, SyntaxPollResult::Kicked);

	// 6. Complete fresh task and install.
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r4 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r4.result, SyntaxPollResult::Ready);
	assert!(mgr.has_syntax(doc_id));
}

/// Must attempt synchronous bootstrap parse when a document is first opened
/// and the tier allows it.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Small files flash un-highlighted text on first open.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_sync_bootstrap_success() {
	// threshold = 10ms, but bootstrap is 5ms -> Ok(syntax)
	let engine = Arc::new(TimeoutSensitiveEngine::new(Duration::from_millis(1)));
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

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// First poll should return Ready immediately
	let r = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));

	assert_eq!(r.result, SyntaxPollResult::Ready);
	assert!(mgr.has_syntax(doc_id));
	assert!(!mgr.has_pending(doc_id));
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);
}

/// Must fall back to background parse if the synchronous bootstrap attempt
/// times out, without setting a cooldown.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Medium files fail to highlight or stall the UI on first open.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_sync_bootstrap_timeout_fallback() {
	// threshold = 10ms, bootstrap is 5ms -> Err(Timeout)
	let engine = Arc::new(TimeoutSensitiveEngine::new(Duration::from_millis(10)));
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

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// First poll should return Kicked (fell back to background)
	let r = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));

	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));

	// Wait for background task to increment parse_count to 2
	let mut iters = 0;
	while engine.parse_count.load(Ordering::SeqCst) < 2 && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	// 1 attempt (sync) + 1 attempt (spawn_blocking kicked immediately)
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 2);

	// Let the background task finish
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r2 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r2.result, SyntaxPollResult::Ready);
}
