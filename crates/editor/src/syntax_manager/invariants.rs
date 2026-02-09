use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::{sleep, timeout};
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::{Syntax, SyntaxError, SyntaxOptions};

use super::*;
use crate::core::document::DocumentId;

/// Mock parsing engine that blocks until explicitly released.
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
		viewport: None,
	}
}

/// Spins until `mgr.any_task_finished()` returns true, up to 100 ms.
async fn wait_for_finish(mgr: &SyntaxManager) {
	timeout(Duration::from_secs(1), async {
		while !mgr.any_task_finished() {
			sleep(Duration::from_millis(1)).await;
		}
	})
	.await
	.expect("Task did not finish in time");
}

/// Must enforce single-flight per document.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Multiple redundant parse tasks for the same document identity.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_single_flight_per_doc() {
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

/// Must not perform unbounded parsing on the UI thread.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`, `SyntaxManager::note_edit_incremental`
/// - Failure symptom: UI freezes or jitters during edits.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_inflight_drained_even_if_doc_marked_clean() {
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

/// Must not regress to a tree older than the currently installed `tree_doc_version`.
///
/// - Enforced in: `should_install_completed_parse`
/// - Failure symptom: Stale trees overwrite newer incrementals, or highlighting stays
///   missing until an exact-version parse completes.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_parse_does_not_overwrite_clean_incremental() {
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

/// Must install completed parses for continuity when the slot is dirty,
/// even if stale, to keep highlighting visible during catch-up reparses.
///
/// - Enforced in: `should_install_completed_parse`
/// - Failure symptom: Highlighting stays missing until an exact-version parse completes.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_stale_install_continuity() {
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

/// Must call `note_edit_incremental` or `note_edit` on every document mutation.
///
/// - Enforced in: `EditorUndoHost::apply_transaction_inner`,
///   `EditorUndoHost::apply_history_op`, `Editor::apply_buffer_edit_plan`
/// - Failure symptom: Debounce is bypassed and background parses run without edit silence.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_note_edit_updates_timestamp() {
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

/// Must skip debounce for bootstrap parses when no syntax tree is installed.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Newly opened documents remain unhighlighted until debounce elapses.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_bootstrap_parse_skips_debounce() {
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

/// Must detect completed inflight tasks from `tick()`, not only from `render()`.
///
/// - Enforced in: `SyntaxManager::drain_finished_inflight` via `Editor::tick`
/// - Failure symptom: Completed parses are not installed while idle until user input
///   triggers rendering.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_idle_tick_polls_inflight_parse() {
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

/// Must bump `syntax_version` whenever the installed tree changes or is dropped.
///
/// - Enforced in: `mark_updated`
/// - Failure symptom: Highlight cache serves stale spans after reparse or retention drop.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_syntax_version_bumps_on_install() {
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

	// Switch to Python - invalidates Rust epoch, new task throttled
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

/// Must tie background task permit lifetime to real thread execution.
///
/// - Enforced in: `TaskCollector::spawn`
/// - Failure symptom: Concurrency cap is violated under churn because permits are
///   released before CPU work ends.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_invalidate_does_not_release_permit_until_task_finishes() {
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

/// Version monotonicity; a completed V5 parse must not clobber a
/// V7 tree that was installed via sync incremental updates.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_monotonic_version_guard() {
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

	// V5 must not clobber V7.
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

/// History edits bypass debounce.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_history_op_bypasses_debounce() {
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

/// Cold eviction and re-bootstrap; a document evicted due to Cold
/// hotness is re-bootstrapped immediately when it becomes visible again.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_eviction_reload() {
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

/// Cold retention throttles work; Cold hotness plus DropWhenHidden
/// invalidates state and throttles new work until the permit is released.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_cold_throttles_work() {
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

	// Permit still held - another doc is throttled
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

/// A task detached while Cold should reattach if the document becomes Visible again.
///
/// - Enforced in: `SyntaxManager::ensure_syntax`
/// - Failure symptom: Document stays Disabled or re-kicks redundant tasks after becoming Visible.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_detached_task_reattach() {
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

	// 1. Visible -> kick task
	mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert!(mgr.has_pending(doc_id));

	// Wait for background task to actually enter engine.parse
	let mut iters = 0;
	while engine.parse_count.load(Ordering::SeqCst) == 0 && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);

	// 2. Visible -> Cold. Should be Disabled and not pending.
	let r1 = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Cold,
		&loader,
	));
	assert_eq!(r1.result, SyntaxPollResult::Disabled);
	assert!(!mgr.has_pending(doc_id));

	// 3. Cold -> Visible. Should be Pending again (reattached).
	let r2 = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
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
	let r3 = mgr.ensure_syntax(make_ctx(
		doc_id,
		1,
		Some(lang_id),
		&content,
		SyntaxHotness::Visible,
		&loader,
	));
	assert_eq!(r3.result, SyntaxPollResult::Ready);
	assert!(mgr.has_syntax(doc_id));
}

/// Highlight rendering must skip spans when `tree_doc_version` differs from
/// the rendered document version.
///
/// - Enforced in: `HighlightTiles::build_tile_spans` (in `crate::render::cache::highlight`)
/// - Failure symptom: Out-of-bounds tree-sitter access can panic during rapid edits.
#[cfg_attr(test, tokio::test)]
pub(crate) async fn test_highlight_skips_stale_tree_version() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// Install syntax at V1.
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

/// Must promote recently visible documents to `Warm` hotness to avoid
/// immediate retention drops.
///
/// - Enforced in: `Editor::ensure_syntax_for_buffers`, `Editor::on_document_close`
/// - Failure symptom: Switching away for one frame drops syntax and causes a flash of
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
