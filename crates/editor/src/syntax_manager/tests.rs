use std::sync::Mutex;

use tokio::sync::oneshot;

use super::*;

struct MockEngine {
	gate: Mutex<Option<oneshot::Receiver<()>>>,
	result: Mutex<Option<Result<Syntax, SyntaxError>>>,
}

impl MockEngine {
	fn new() -> Self {
		Self {
			gate: Mutex::new(None),
			result: Mutex::new(None),
		}
	}

	fn set_gate(&self, rx: oneshot::Receiver<()>) {
		*self.gate.lock().unwrap() = Some(rx);
	}

	fn set_result(&self, res: Result<Syntax, SyntaxError>) {
		*self.result.lock().unwrap() = Some(res);
	}
}

impl SyntaxEngine for MockEngine {
	fn parse(
		&self,
		_content: ropey::RopeSlice<'_>,
		_lang: LanguageId,
		_loader: &LanguageLoader,
		_opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		if let Some(rx) = self.gate.lock().unwrap().take() {
			let _ = rx.blocking_recv();
		}

		self.result
			.lock()
			.unwrap()
			.take()
			.unwrap_or(Err(SyntaxError::Timeout))
	}
}

/// Verifies that an inflight task is polled and its result handled even if the
/// document was marked clean (dirty=false) by a synchronous parse while the
/// background task was still running.
#[tokio::test]
async fn test_inflight_drained_even_if_doc_marked_clean() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		s: TierCfg {
			debounce: Duration::ZERO,
			cooldown_on_timeout: Duration::ZERO,
			..TieredSyntaxPolicy::default().s
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));

	if let Some(entry) = mgr.entries.get_mut(&doc_id) {
		entry.slot.dirty = false;
	}

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert!(!mgr.has_pending(doc_id));
	assert!(matches!(
		poll.result,
		SyntaxPollResult::Ready | SyntaxPollResult::CoolingDown
	));
}

/// Verifies that switching a document's language discards any existing syntax
/// tree and aborts inflight tasks.
#[tokio::test]
async fn test_language_switch_discards_old_parse() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		s: TierCfg {
			debounce: Duration::ZERO,
			cooldown_on_timeout: Duration::ZERO,
			..TieredSyntaxPolicy::default().s
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id_old = loader.language_for_name("rust").unwrap();
	let lang_id_new = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	let dummy_syntax = Syntax::new(
		content.slice(..),
		lang_id_old,
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.unwrap();
	engine.set_result(Ok(dummy_syntax));

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_old),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_new),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert!(mgr.syntax_for_doc(doc_id).is_none());
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
}

/// Verifies that the memory retention policy correctly evicts syntax trees
/// when a document becomes `Cold` and that the eviction clears the dirty flag.
#[tokio::test]
async fn test_dropwhenhidden_discards_completed_parse() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		l: TierCfg {
			retention_hidden: RetentionPolicy::DropWhenHidden,
			debounce: Duration::ZERO,
			cooldown_on_timeout: Duration::ZERO,
			..TieredSyntaxPolicy::default().l
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from(" ".repeat(2 * 1024 * 1024));

	let dummy_syntax = Syntax::new(
		content.slice(..),
		lang_id,
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.unwrap();
	engine.set_result(Ok(dummy_syntax));

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
	});

	assert!(mgr.syntax_for_doc(doc_id).is_none());
	assert!(!mgr.is_dirty(doc_id));
	assert!(poll.updated);
}

/// Exhaustive truth table for the stale-inflight install guard.
///
/// The critical regression case is `(version_match=false, dirty=false,
/// has_current=true)`, a clean incremental tree at V1 must NOT be
/// overwritten by a stale background parse from V0.
#[test]
fn test_stale_parse_does_not_overwrite_clean_incremental() {
	use super::should_install_completed_parse;

	// (done_version, current_tree_version, target_version, slot_dirty, expected)
	let cases: [(u64, Option<u64>, u64, bool, bool); 9] = [
		(5, Some(3), 10, false, false), // Clean tree + stale result → MUST NOT install.
		(5, Some(3), 5, false, true),   // Exact version match → always install.
		(5, Some(3), 5, true, true),
		(5, None, 5, false, true),
		(5, None, 5, true, true),
		(5, Some(3), 10, true, true), // Dirty slot → install stale for catch-up continuity.
		(5, None, 10, true, true),
		(5, None, 10, false, true), // No current syntax → install stale for bootstrap.
		(0, Some(1), 1, false, false), // Clean incremental (V1) must not be clobbered by stale background (V0)
	];

	for (done_version, current_tree_version, target_version, dirty, expected) in cases {
		let result = should_install_completed_parse(
			done_version,
			current_tree_version,
			target_version,
			dirty,
		);
		assert_eq!(
			result, expected,
			"should_install_completed_parse(done={done_version}, current={current_tree_version:?}, target={target_version}, dirty={dirty}) = {result}, expected {expected}"
		);
	}
}

/// Verifies that a completed parse for an older document version is still installed
/// if the document is currently dirty, ensuring highlighting continuity during
/// rapid edits.
#[tokio::test]
async fn test_stale_install_continuity() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		s: TierCfg {
			debounce: Duration::ZERO,
			cooldown_on_timeout: Duration::ZERO,
			..TieredSyntaxPolicy::default().s
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let dummy_syntax = Syntax::new(
		content.slice(..),
		lang_id,
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.unwrap();
	engine.set_result(Ok(dummy_syntax));

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert!(mgr.syntax_for_doc(doc_id).is_some());
	assert!(poll.updated);
	assert!(mgr.is_dirty(doc_id));
}

/// Verifies that bootstrap parses (first parse for a document) skip the debounce
/// timer to ensure immediate highlighting when a file is opened.
#[tokio::test]
async fn test_bootstrap_parse_skips_debounce() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));
}

#[test]
fn test_note_edit_updates_timestamp() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);

	mgr.note_edit(doc_id);
	let t1 = mgr.entries.get(&doc_id).unwrap().sched.last_edit_at;

	std::thread::sleep(Duration::from_millis(1));
	mgr.note_edit(doc_id);
	let t2 = mgr.entries.get(&doc_id).unwrap().sched.last_edit_at;

	assert!(t2 > t1);
}

/// Verifies that the editor tick can detect completed tasks and trigger redraws
/// even when the document is not currently being rendered.
#[tokio::test]
async fn test_idle_tick_polls_inflight_parse() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		s: TierCfg {
			debounce: Duration::ZERO,
			..TieredSyntaxPolicy::default().s
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert!(mgr.has_pending(doc_id));
	assert!(!mgr.any_task_finished());

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	assert!(mgr.any_task_finished());
}

/// Verifies that the manager enforces a single-flight policy per document,
/// preventing multiple redundant parse tasks from running simultaneously for
/// the same document ID.
#[tokio::test]
async fn test_single_flight_per_doc() {
	let engine = Arc::new(MockEngine::new());
	let (_tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		s: TierCfg {
			debounce: Duration::ZERO,
			..TieredSyntaxPolicy::default().s
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	let poll2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert_eq!(poll2.result, SyntaxPollResult::Pending);
	assert_eq!(mgr.pending_count(), 1);
}

/// Verifies that installing a new syntax tree increments the slot's version,
/// allowing the highlight cache to detect changes.
#[tokio::test]
async fn test_syntax_version_bumps_on_install() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	mgr.set_policy(TieredSyntaxPolicy {
		s: TierCfg {
			debounce: Duration::ZERO,
			cooldown_on_timeout: Duration::ZERO,
			..TieredSyntaxPolicy::default().s
		},
		..TieredSyntaxPolicy::default()
	});

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let dummy_syntax = Syntax::new(
		content.slice(..),
		lang_id,
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.unwrap();
	engine.set_result(Ok(dummy_syntax));

	let v0 = mgr.syntax_version(doc_id);
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::Ready);
	assert!(poll.updated);
	assert!(mgr.syntax_version(doc_id) > v0);
}

/// Verifies that completed tasks produced under a stale configuration (options key)
/// are discarded and a reparse is triggered.
#[tokio::test]
async fn test_opts_mismatch_never_installs() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.injections = InjectionPolicy::Eager;
	mgr.set_policy(policy.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	policy.s.injections = InjectionPolicy::Disabled;
	mgr.set_policy(policy);

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert_eq!(poll.result, SyntaxPollResult::Kicked);
	assert!(mgr.is_dirty(doc_id));
}

/// Verifies that a configuration change (e.g., tier threshold or injection policy)
/// immediately aborts any active background task for that document.
#[tokio::test]
async fn test_opts_mismatch_aborts_inflight() {
	let engine = Arc::new(MockEngine::new());
	let (_tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.injections = InjectionPolicy::Eager;
	mgr.set_policy(policy.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	policy.s.injections = InjectionPolicy::Disabled;
	mgr.set_policy(policy);

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	assert_eq!(poll.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));
}

#[test]
#[should_panic(expected = "TieredSyntaxPolicy: s_max (20) must be <= m_max (10)")]
fn test_set_policy_validates_thresholds() {
	let mut mgr = SyntaxManager::default();
	let policy = TieredSyntaxPolicy {
		s_max_bytes_inclusive: 20,
		m_max_bytes_inclusive: 10,
		..TieredSyntaxPolicy::default()
	};
	mgr.set_policy(policy);
}

/// Verifies that aborting an inflight task releases its semaphore permit.
///
/// This test fills the concurrency limit (1) with a stalled task, then triggers
/// an abort via language switch. It asserts that a subsequent task can be kicked,
/// proving the permit was released.
#[tokio::test]
async fn test_abort_releases_permit() {
	let engine = Arc::new(MockEngine::new());
	let (tx_stall, rx_stall) = oneshot::channel();
	engine.set_gate(rx_stall);

	// Max concurrency = 1
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id_1 = loader.language_for_name("rust").unwrap();
	let lang_id_2 = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// 1. Kick off first task (stalled)
	let poll1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_1),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll1.result, SyntaxPollResult::Kicked);

	// 2. Trigger abort via language switch
	// This aborts the inflight task. The `tokio::task::spawn_blocking` closure should
	// be dropped, releasing the `OwnedSemaphorePermit`.
	let poll2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_2),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	// Result should be Kicked again (if permit was released) or Throttled (if leaked)
	assert_eq!(poll2.result, SyntaxPollResult::Kicked);

	// Clean up the stalled task to avoid leaked task warnings
	let _ = tx_stall.send(());
}

/// Verifies that completing a task and calling drain_finished_inflight releases
/// the permit even if the document is not re-polled via ensure_syntax.
#[tokio::test]
async fn test_drain_releases_permit_without_repoll() {
	let engine = Arc::new(MockEngine::new());
	let (tx_stall, rx_stall) = oneshot::channel();
	engine.set_gate(rx_stall);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id_a = DocumentId(1);
	let doc_id_b = DocumentId(2);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Kick off task for doc A
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: doc_id_a,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id_a));

	// 2. Complete task A
	let _ = tx_stall.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	// 3. Drain A
	assert!(mgr.drain_finished_inflight());
	assert!(!mgr.has_pending(doc_id_a));

	// 4. Kick task B. This should succeed if A's permit was released.
	let poll_b = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: doc_id_b,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll_b.result, SyntaxPollResult::Kicked);
}

/// Verifies that switching a document's language clears any stale completed error
/// (like a timeout) and allows an immediate re-parse.
#[tokio::test]
async fn test_language_switch_clears_completed_error() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	// Set long cooldown to ensure we'd be blocked if not cleared
	policy.s.cooldown_on_timeout = Duration::from_secs(60);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id_old = loader.language_for_name("rust").unwrap();
	let lang_id_new = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// 1. Arrange a timeout error in 'completed'
	engine.set_result(Err(SyntaxError::Timeout));
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_old),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	// Poll to move result to completed and set cooldown
	tokio::time::sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	// Verify it's cooling down
	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_old),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::CoolingDown);

	// 2. Switch language
	let poll_new = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_new),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	// Should be Kicked, not CoolingDown
	assert_eq!(poll_new.result, SyntaxPollResult::Kicked);
}
