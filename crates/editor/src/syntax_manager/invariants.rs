//! Machine-checkable invariant proofs for [`super::SyntaxManager`].
//!
//! Each invariant is expressed as a `pub(crate) async fn inv_*()` proof function,
//! wrapped by a `#[cfg_attr(test, tokio::test)] pub(crate) async fn test_*()`
//! that doubles as an intra-doc link target for the anchor module-level docs.
//!
//! Shared test infrastructure ([`MockEngine`], [`EngineGuard`]) lives here so
//! both this module and the sibling `tests` module can reuse it.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::sleep;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::{Syntax, SyntaxError, SyntaxOptions};

use super::*;
use crate::core::document::DocumentId;

// Mock parsing engine that blocks until explicitly released.
///
/// Allows tests to control task scheduling deterministically by gating
/// `parse` calls behind a [`Notify`] barrier.
pub(crate) struct MockEngine {
	pub(crate) parse_count: AtomicUsize,
	pub(crate) result: Arc<parking_lot::Mutex<std::result::Result<Syntax, String>>>,
	pub(crate) notify: Arc<Notify>,
}

impl MockEngine {
	pub(crate) fn new() -> Self {
		let loader = LanguageLoader::from_embedded();
		let lang = loader.language_for_name("rust").unwrap();
		let syntax = Syntax::new(
			Rope::from("").slice(..),
			lang,
			&loader,
			SyntaxOptions::default(),
		)
		.unwrap();

		Self {
			parse_count: AtomicUsize::new(0),
			result: Arc::new(parking_lot::Mutex::new(Ok(syntax))),
			notify: Arc::new(Notify::new()),
		}
	}

	pub(crate) fn set_result(&self, res: std::result::Result<Syntax, String>) {
		*self.result.lock() = res;
	}

	/// Allows one pending parse to proceed.
	pub(crate) fn proceed(&self) {
		self.notify.notify_one();
	}

	/// Allows all pending parses to proceed immediately.
	pub(crate) fn proceed_all(&self) {
		for _ in 0..100 {
			self.notify.notify_one();
		}
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
		self.parse_count.fetch_add(1, Ordering::SeqCst);
		futures::executor::block_on(self.notify.notified());

		match &*self.result.lock() {
			Ok(s) => Ok(s.clone()),
			Err(e) => {
				if e == "timeout" {
					Err(SyntaxError::Timeout)
				} else {
					Err(SyntaxError::Parse(e.clone()))
				}
			}
		}
	}
}

/// RAII guard that unblocks all pending parses on drop, preventing test hangs.
pub(crate) struct EngineGuard(pub(crate) Arc<MockEngine>);

impl Drop for EngineGuard {
	fn drop(&mut self) {
		self.0.proceed_all();
	}
}

/// Convenience: creates a standard [`EnsureSyntaxContext`] for tests.
fn make_ctx<'a>(
	doc_id: DocumentId,
	doc_version: u64,
	language_id: Option<LanguageId>,
	content: &'a Rope,
	hotness: SyntaxHotness,
	loader: &'a Arc<LanguageLoader>,
) -> EnsureSyntaxContext<'a> {
	EnsureSyntaxContext {
		doc_id,
		doc_version,
		language_id,
		content,
		hotness,
		loader,
	}
}

/// Spins until `mgr.any_task_finished()` returns true, up to 100 ms.
async fn wait_for_finish(mgr: &SyntaxManager) {
	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished(), "Task did not finish in time");
}

/// Invariant: single-flight per document.
pub(crate) async fn inv_single_flight_per_doc() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let r1 = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	let r2 = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r2.result, SyntaxPollResult::Pending);

	engine.proceed();
	wait_for_finish(&mgr).await;
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);
}

/// Invariant: no unbounded UI-thread parsing — inflight tasks are drained
/// even when the document has been marked clean externally.
pub(crate) async fn inv_inflight_drained_even_if_doc_marked_clean() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_pending(doc_id));

	mgr.force_clean(doc_id);
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	assert!(!mgr.has_pending(doc_id));

	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_syntax(doc_id));
}

/// Invariant: monotonic version / stale guard — a stale V1 parse MUST NOT
/// overwrite a clean V2 incremental tree.
pub(crate) async fn inv_stale_parse_does_not_overwrite_clean_incremental() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish initial tree at V1
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));

	// Kick background reparse at V1 (stale)
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_pending(doc_id));

	// Sync incremental catchup to V2
	mgr.note_edit_incremental(
		doc_id,
		2,
		&content,
		&content,
		&ChangeSet::new(content.slice(..)),
		&loader,
		EditSource::Typing,
	);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(2));
	assert!(!mgr.is_dirty(doc_id));

	// Stale V1 reparse completes
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();

	mgr.ensure_syntax(make_ctx(
		doc_id,
		2,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(
		mgr.syntax_doc_version(doc_id),
		Some(2),
		"Stale V1 must not overwrite clean V2"
	);
}

/// Invariant: stale install for continuity — a stale parse is installed when
/// the slot is dirty, providing some highlighting while a catch-up reparse runs.
pub(crate) async fn inv_stale_install_continuity() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Kick parse at V1
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));

	// Edit to V2 while parse is inflight
	mgr.note_edit(doc_id, EditSource::Typing);

	// Complete V1 parse
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// Poll - should install V1 even if stale because slot is dirty
	mgr.ensure_syntax(make_ctx(
		doc_id,
		2,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));
	assert!(
		mgr.is_dirty(doc_id),
		"Should remain dirty for catch-up reparse"
	);
}

/// Invariant: edit notification updates debounce timestamp.
pub(crate) async fn inv_note_edit_updates_timestamp() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(100);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish initial tree
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_syntax(doc_id));

	// Note edit (Typing)
	mgr.note_edit(doc_id, EditSource::Typing);
	assert!(mgr.is_dirty(doc_id));

	// Poll immediately - should be Pending (debounced)
	let r = mgr.ensure_syntax(make_ctx(
		doc_id,
		2,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Pending);

	// Wait for debounce
	sleep(Duration::from_millis(150)).await;
	let r2 = mgr.ensure_syntax(make_ctx(
		doc_id,
		2,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r2.result, SyntaxPollResult::Kicked);
}

/// Invariant: bootstrap parses skip debounce.
pub(crate) async fn inv_bootstrap_parse_skips_debounce() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_secs(60);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let r = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Invariant: tick detects completed tasks.
pub(crate) async fn inv_idle_tick_polls_inflight_parse() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_pending(doc_id));
	assert!(!mgr.any_task_finished());

	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	assert!(!mgr.has_pending(doc_id));
}

/// Invariant: syntax_version monotonicity — bumps on install.
pub(crate) async fn inv_syntax_version_bumps_on_install() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let v0 = mgr.syntax_version(doc_id);

	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));

	let v1 = mgr.syntax_version(doc_id);
	assert!(v1 > v0);
}

/// Invariant: language change discards old parse.
pub(crate) async fn inv_language_switch_discards_old_parse() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_rust = loader.language_for_name("rust").unwrap();
	let lang_py = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// Kick Rust parse
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_rust),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));

	// Switch to Python — invalidates Rust epoch, new task throttled
	let r = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_py),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Throttled);

	// Rust result ready but discarded
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_py),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Kicked);

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_py),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Ready);
	assert!(mgr.has_syntax(doc_id));
}

/// Invariant: permit lifetime tied to thread — invalidation does not release
/// the semaphore permit early; only task completion does.
pub(crate) async fn inv_invalidate_does_not_release_permit_until_task_finishes() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_rust = loader.language_for_name("rust").unwrap();
	let lang_py = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// Kick task for Doc 1
	mgr.ensure_syntax(make_ctx(
		DocumentId(1),
		1,
		Some(lang_rust),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(mgr.pending_count(), 1);

	// Switch language -> invalidates epoch, but permit still held
	let r = mgr.ensure_syntax(make_ctx(
		DocumentId(1),
		1,
		Some(lang_py),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Throttled);

	// Allow first task to finish
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();

	// Now the new task can be kicked
	let r = mgr.ensure_syntax(make_ctx(
		DocumentId(1),
		1,
		Some(lang_py),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Invariant: version monotonicity — a completed V5 parse MUST NOT clobber a
/// V7 tree that was installed via sync incremental updates.
pub(crate) async fn inv_monotonic_version_guard() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish syntax at V5
	mgr.ensure_syntax(make_ctx(
		doc_id,
		5,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(
		doc_id,
		5,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(5));

	// Kick another parse at V5 (slow redundant one)
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(make_ctx(
		doc_id,
		5,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_pending(doc_id));

	// Advance to V7 via sync incremental updates
	mgr.note_edit_incremental(
		doc_id,
		6,
		&content,
		&content,
		&ChangeSet::new(content.slice(..)),
		&loader,
		EditSource::Typing,
	);
	mgr.note_edit_incremental(
		doc_id,
		7,
		&content,
		&content,
		&ChangeSet::new(content.slice(..)),
		&loader,
		EditSource::Typing,
	);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(7));

	// Complete the V5 parse
	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	// V5 MUST NOT clobber V7
	mgr.ensure_syntax(make_ctx(
		doc_id,
		10,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(
		mgr.syntax_doc_version(doc_id),
		Some(7),
		"V5 should not clobber V7"
	);
}

/// Invariant: history edits bypass debounce.
pub(crate) async fn inv_history_op_bypasses_debounce() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_secs(60);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish initial tree
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_syntax(doc_id));

	// Note edit (History)
	mgr.note_edit(doc_id, EditSource::History);
	assert!(mgr.is_dirty(doc_id));

	// Poll immediately - should NOT be debounced because History
	let r = mgr.ensure_syntax(make_ctx(
		doc_id,
		2,
		Some(lang),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Invariant: cold eviction + re-bootstrap — a document evicted due to Cold
/// hotness is re-bootstrapped immediately when it becomes visible again.
pub(crate) async fn inv_cold_eviction_reload() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Establish syntax
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	engine.proceed();
	wait_for_finish(&mgr).await;

	mgr.drain_finished_inflight();
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_syntax(doc_id));

	// Trigger eviction
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Cold,
		&loader,
	));
	assert!(!mgr.has_syntax(doc_id));

	// Become visible again - should Kick bootstrap immediately
	let poll = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
}

/// Invariant: cold retention throttles work — Cold hotness + DropWhenHidden
/// invalidates state and throttles new work until the permit is released.
pub(crate) async fn inv_cold_throttles_work() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Start inflight parse
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_pending(doc_id));

	// Drop hotness to Cold - should invalidate and return Disabled
	let poll = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Cold,
		&loader,
	));
	assert_eq!(poll.result, SyntaxPollResult::Disabled);
	assert!(!mgr.has_pending(doc_id));

	// Permit still held — another doc is throttled
	let poll2 = mgr.ensure_syntax(make_ctx(
		DocumentId(2),
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(poll2.result, SyntaxPollResult::Throttled);

	engine.proceed();
	wait_for_finish(&mgr).await;
	mgr.drain_finished_inflight();

	let poll3 = mgr.ensure_syntax(make_ctx(
		DocumentId(2),
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(poll3.result, SyntaxPollResult::Kicked);
}

// ---------------------------------------------------------------------------
// Test wrappers (intra-doc link targets)
// ---------------------------------------------------------------------------

/// Proof: single-flight per document.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_single_flight_per_doc() {
	inv_single_flight_per_doc().await;
}

/// Proof: inflight tasks drained even when document is clean.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_inflight_drained_even_if_doc_marked_clean() {
	inv_inflight_drained_even_if_doc_marked_clean().await;
}

/// Proof: stale parse does not overwrite clean incremental tree.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_parse_does_not_overwrite_clean_incremental() {
	inv_stale_parse_does_not_overwrite_clean_incremental().await;
}

/// Proof: stale parse installed for rendering continuity.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_install_continuity() {
	inv_stale_install_continuity().await;
}

/// Proof: edit notification updates debounce timestamp.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_note_edit_updates_timestamp() {
	inv_note_edit_updates_timestamp().await;
}

/// Proof: bootstrap parse skips debounce.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_bootstrap_parse_skips_debounce() {
	inv_bootstrap_parse_skips_debounce().await;
}

/// Proof: tick detects completed tasks.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_idle_tick_polls_inflight_parse() {
	inv_idle_tick_polls_inflight_parse().await;
}

/// Proof: syntax_version bumps on install.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_syntax_version_bumps_on_install() {
	inv_syntax_version_bumps_on_install().await;
}

/// Proof: language switch discards old parse.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_language_switch_discards_old_parse() {
	inv_language_switch_discards_old_parse().await;
}

/// Proof: permit lifetime tied to thread execution.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_invalidate_does_not_release_permit_until_task_finishes() {
	inv_invalidate_does_not_release_permit_until_task_finishes().await;
}

/// Proof: version monotonicity guard.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_monotonic_version_guard() {
	inv_monotonic_version_guard().await;
}

/// Proof: history edits bypass debounce.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_history_op_bypasses_debounce() {
	inv_history_op_bypasses_debounce().await;
}

/// Proof: cold eviction + re-bootstrap.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_eviction_reload() {
	inv_cold_eviction_reload().await;
}

/// Proof: cold retention throttles work.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_throttles_work() {
	inv_cold_throttles_work().await;
}
