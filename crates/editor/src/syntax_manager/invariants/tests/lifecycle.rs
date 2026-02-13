use super::*;

#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_single_flight_per_doc() {
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
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let r1 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	let r2 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang_id), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r2.result, SyntaxPollResult::Pending);

	engine.proceed();
	wait_for_finish(&mgr).await;
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);
}

/// Must not perform unbounded parsing on the UI thread.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`, `SyntaxManager::note_edit_incremental`
/// - Failure symptom: UI freezes or jitters during edits.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_inflight_drained_even_if_doc_marked_clean() {
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
	let content = Rope::from("test");

	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(doc_id));

	mgr.force_clean(doc_id);
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	assert!(!mgr.has_pending(doc_id));

	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_syntax(doc_id));
}

/// Must not regress to a tree older than the currently installed `tree_doc_version`.
///
/// - Enforced in: `should_install_completed_parse`
/// - Failure symptom: Stale trees overwrite newer incrementals, or highlighting stays
///   missing until an exact-version parse completes.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_parse_does_not_overwrite_clean_incremental() {
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
	let content = Rope::from("test");

	// Establish initial tree at V1
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));

	// Kick background reparse at V1 (stale)
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(doc_id));

	// Sync incremental catchup to V2
	mgr.note_edit_incremental(doc_id, 2, &content, &content, &ChangeSet::new(content.slice(..)), &loader, EditSource::Typing);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(2));
	assert!(!mgr.is_dirty(doc_id));

	// Stale V1 reparse completes
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();

	mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(2), "Stale V1 must not overwrite clean V2");
}

/// Must install completed parses for continuity when the slot is dirty,
/// even if stale, to keep highlighting visible during catch-up reparses.
///
/// - Enforced in: `should_install_completed_parse`
/// - Failure symptom: Highlighting stays missing until an exact-version parse completes.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_install_continuity() {
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
	let content = Rope::from("test");

	// Kick parse at V1
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));

	// Edit to V2 while parse is inflight
	mgr.note_edit(doc_id, EditSource::Typing);

	// Complete V1 parse
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// Poll - should install V1 even if stale because slot is dirty
	mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));
	assert!(mgr.is_dirty(doc_id), "Should remain dirty for catch-up reparse");
}

/// Must call `note_edit_incremental` or `note_edit` on every document mutation.
///
/// - Enforced in: `EditorUndoHost::apply_transaction_inner`,
///   `EditorUndoHost::apply_history_op`, `Editor::apply_buffer_edit_plan`
/// - Failure symptom: Debounce is bypassed and background parses run without edit silence.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_note_edit_updates_timestamp() {
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
	policy.s.debounce = Duration::from_millis(100);
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

	// Note edit (Typing)
	mgr.note_edit(doc_id, EditSource::Typing);
	assert!(mgr.is_dirty(doc_id));

	// Poll immediately - should be Pending (debounced)
	let r = mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Pending);

	// Wait for debounce
	sleep(Duration::from_millis(150)).await;
	let r2 = mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r2.result, SyntaxPollResult::Kicked);
}

/// Must skip debounce for bootstrap parses when no syntax tree is installed.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Newly opened documents remain unhighlighted until debounce elapses.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_bootstrap_parse_skips_debounce() {
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

	let r = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Must detect completed inflight tasks from `tick()`, not only from `render()`.
///
/// - Enforced in: `SyntaxManager::drain_finished_inflight` via `Editor::tick`
/// - Failure symptom: Completed parses are not installed while idle until user input
///   triggers rendering.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_idle_tick_polls_inflight_parse() {
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
	let content = Rope::from("test");

	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(mgr.has_pending(doc_id));
	assert!(!mgr.any_task_finished());

	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	assert!(!mgr.has_pending(doc_id));
}

/// Must bump `syntax_version` whenever the installed tree changes or is dropped.
///
/// - Enforced in: `mark_updated`
/// - Failure symptom: Highlight cache serves stale spans after reparse or retention drop.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_syntax_version_bumps_on_install() {
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
	let content = Rope::from("test");

	let v0 = mgr.syntax_version(doc_id);

	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));

	let v1 = mgr.syntax_version(doc_id);
	assert!(v1 > v0);
}
