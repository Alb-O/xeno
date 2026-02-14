use xeno_primitives::Transaction;
use xeno_primitives::transaction::Change;

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
/// * Enforced in: `SyntaxManager::ensure_syntax`, `SyntaxManager::note_edit_incremental`
/// * Failure symptom: UI freezes or jitters during edits.
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
/// * Enforced in: `should_install_completed_parse`
/// * Failure symptom: Stale trees overwrite newer incrementals, or highlighting stays
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

/// Must install stale completed parses for continuity when the slot is dirty
/// and no resident tree exists, to keep highlighting visible during catch-up reparses.
///
/// * Enforced in: `should_install_completed_parse`
/// * Failure symptom: Highlighting stays missing until an exact-version parse completes.
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

/// Must skip stale non-viewport installs when they would break projection continuity.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Undo applies a stale intermediate full/incremental tree that
///   clears projection context, causing a broken repaint before the exact parse lands.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_incremental_parse_skips_install_when_projection_would_break() {
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

	let content_v1 = Rope::from("hello");

	// Establish initial full tree at V1.
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content_v1, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content_v1, SyntaxHotness::Visible, &loader));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));

	// History edit V1 -> V2; pending stays anchored to V1.
	let tx1 = Transaction::change(
		content_v1.slice(..),
		[Change {
			start: 5,
			end: 5,
			replacement: Some(" world".to_string()),
		}],
	);
	let mut content_v2 = content_v1.clone();
	tx1.apply(&mut content_v2);
	mgr.note_edit_incremental(doc_id, 2, &content_v1, &content_v2, tx1.changes(), &loader, EditSource::History);

	// Kick background incremental parse targeting V2.
	let kick = mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content_v2, SyntaxHotness::Visible, &loader));
	assert_eq!(kick.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_bg(doc_id));

	// Another history edit V2 -> V3 while V2 parse is still in-flight.
	// Pending remains anchored to V1, so installing stale V2 would lose projection.
	let tx2 = Transaction::change(
		content_v2.slice(..),
		[Change {
			start: content_v2.len_chars(),
			end: content_v2.len_chars(),
			replacement: Some("!".to_string()),
		}],
	);
	let mut content_v3 = content_v2.clone();
	tx2.apply(&mut content_v3);
	mgr.note_edit_incremental(doc_id, 3, &content_v2, &content_v3, tx2.changes(), &loader, EditSource::History);

	// Complete stale V2 incremental parse.
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let _ = mgr.ensure_syntax(make_ctx(doc_id, 3, Some(lang), &content_v3, SyntaxHotness::Visible, &loader));

	let selected = mgr
		.syntax_for_viewport(doc_id, 3, 0..content_v3.len_bytes() as u32)
		.expect("resident tree must remain available");
	assert_eq!(selected.tree_doc_version, 1, "stale V2 parse must not replace the V1 projection baseline");
	assert!(
		mgr.highlight_projection_ctx_for(doc_id, selected.tree_doc_version, 3).is_some(),
		"projection continuity must remain available after skipping stale install"
	);
}

/// Must skip stale full-result installs when they don't advance resident version.
///
/// * Enforced in: `should_install_completed_parse`
/// * Failure symptom: Large-file edits trigger a delayed no-op repaint before the real catch-up repaint.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_same_version_parse_does_not_reinstall() {
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

	// Establish initial full tree at V1.
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));

	let tree_id_before = mgr
		.syntax_for_viewport(doc_id, 1, 0..content.len_bytes() as u32)
		.expect("full tree must be present")
		.tree_id;

	// Kick a background parse still targeting V1.
	mgr.mark_dirty(doc_id);
	let r1 = mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	// Document advances to V2 while V1 parse is in-flight.
	mgr.note_edit(doc_id, EditSource::Typing);

	// Complete stale V1 parse.
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let outcome = mgr.ensure_syntax(make_ctx(doc_id, 2, Some(lang), &content, SyntaxHotness::Visible, &loader));
	assert!(
		!outcome.updated,
		"same-version stale completion should not trigger an intermediate install/repaint"
	);

	let tree_id_after = mgr
		.syntax_for_viewport(doc_id, 2, 0..content.len_bytes() as u32)
		.expect("full tree must remain present")
		.tree_id;
	assert_eq!(tree_id_after, tree_id_before);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));
}

/// Must call `note_edit_incremental` or `note_edit` on every document mutation.
///
/// * Enforced in: `EditorUndoHost::apply_transaction_inner`,
///   `EditorUndoHost::apply_history_op`, `Editor::apply_buffer_edit_plan`
/// * Failure symptom: Debounce is bypassed and background parses run without edit silence.
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
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Newly opened documents remain unhighlighted until debounce elapses.
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
/// * Enforced in: `SyntaxManager::drain_finished_inflight` via `Editor::tick`
/// * Failure symptom: Completed parses are not installed while idle until user input
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
/// * Enforced in: `mark_updated`
/// * Failure symptom: Highlight cache serves stale spans after reparse or retention drop.
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

/// Must rotate full-tree identity when sync incremental updates mutate the tree.
///
/// * Enforced in: `SyntaxManager::note_edit_incremental`
/// * Failure symptom: Highlight tiles keyed by tree identity persist stale spans through edits.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_full_tree_id_rotates_on_sync_incremental_update() {
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
	let old_content = Rope::from("fn main() {}\n");

	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &old_content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &old_content, SyntaxHotness::Visible, &loader));

	let tree_id_before = mgr
		.syntax_for_viewport(doc_id, 1, 0..old_content.len_bytes() as u32)
		.expect("full tree must be present at V1")
		.tree_id;

	let tx = Transaction::change(
		old_content.slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("let _x = 1;\n".into()),
		}],
	);
	let mut new_content = old_content.clone();
	tx.apply(&mut new_content);

	mgr.note_edit_incremental(doc_id, 2, &old_content, &new_content, tx.changes(), &loader, EditSource::Typing);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(2));

	let tree_id_after = mgr
		.syntax_for_viewport(doc_id, 2, 0..new_content.len_bytes() as u32)
		.expect("full tree must remain present after sync incremental update")
		.tree_id;

	assert_ne!(tree_id_after, tree_id_before);
}

/// Must monotonically advance `syntax_version` across viewport install, full install,
/// and retention drop via `sweep_retention`.
///
/// * Enforced in: `mark_updated`, `apply_retention`
/// * Failure symptom: Highlight cache serves stale spans after a state transition.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_syntax_version_monotonic_across_install_and_retention() {
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
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	let v0 = mgr.syntax_version(doc_id);
	assert_eq!(v0, 0, "no entry yet → version 0");

	// Viewport install: seed a viewport completion manually.
	{
		let syntax = Syntax::new(content.slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = true;
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: OptKey {
				injections: InjectionPolicy::Disabled,
			},
			result: Ok(syntax),
			class: TaskClass::Viewport,
			elapsed: Duration::ZERO,
			viewport_key: Some(ViewportKey(0)),
			viewport_lane: Some(crate::syntax_manager::scheduling::ViewportLane::Urgent),
		});
	}
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	let v1 = mgr.syntax_version(doc_id);
	assert!(v1 > v0, "viewport install must bump version: {v1} > {v0}");

	// Full install: kick + complete a full parse.
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(doc_id, 1, Some(lang), &content, SyntaxHotness::Visible, &loader));
	let v2 = mgr.syntax_version(doc_id);
	assert!(v2 > v1, "full install must bump version: {v2} > {v1}");

	// Retention drop via sweep_retention.
	mgr.sweep_retention(Instant::now(), |_| SyntaxHotness::Cold);
	let v3 = mgr.syntax_version(doc_id);
	assert!(v3 > v2, "retention drop must bump version: {v3} > {v2}");
}

/// Must flush completed queue for cold docs with `parse_when_hidden = false`
/// during `sweep_retention`, preventing unbounded memory accumulation.
///
/// * Enforced in: `SyntaxManager::sweep_retention`
/// * Failure symptom: Completed `Syntax` trees accumulate in the queue for hidden
///   docs, growing memory without bound.
#[cfg_attr(test, test)]
pub(crate) fn test_sweep_retention_flushes_completed_for_cold_disabled_docs() {
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

	// Set last_tier manually (normally done by ensure_syntax) and push a completion.
	let entry = mgr.entry_mut(doc_id);
	entry.last_tier = Some(SyntaxTier::S);
	entry.sched.completed.push_back(CompletedSyntaxTask {
		doc_version: 1,
		lang_id: lang,
		opts: OptKey {
			injections: InjectionPolicy::Disabled,
		},
		result: Err(SyntaxError::Timeout),
		class: TaskClass::Full,
		elapsed: Duration::ZERO,
		viewport_key: None,
		viewport_lane: None,
	});
	assert!(!entry.sched.completed.is_empty());

	// Sweep as Cold → should flush the completed queue.
	mgr.sweep_retention(Instant::now(), |_| SyntaxHotness::Cold);

	let entry = mgr.entry_mut(doc_id);
	assert!(entry.sched.completed.is_empty(), "completed queue must be flushed for cold disabled docs");
}
