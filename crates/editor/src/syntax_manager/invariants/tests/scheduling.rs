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

	// Trigger eviction
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Cold, &loader));
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

/// A task detached while Cold should reattach if the document becomes Visible again.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Document stays Disabled or re-kicks redundant tasks after becoming Visible.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_detached_task_reattach() {
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

	// 2. Visible -> Cold. Should be Disabled and not pending.
	let r1 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Cold, &loader));
	assert_eq!(r1.result, SyntaxPollResult::Disabled);
	assert!(!mgr.has_pending(doc_id));

	// 3. Cold -> Visible. Should be Pending again (reattached).
	let r2 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r2.result, SyntaxPollResult::Pending);
	assert!(mgr.has_pending(doc_id));
	assert!(mgr.has_inflight_task(doc_id), "task should be reattached");

	// 4. Ensure no second task was kicked
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);

	// 5. Complete task
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// 6. Install
	let r3 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r3.result, SyntaxPollResult::Ready);
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
