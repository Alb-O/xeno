use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::sleep;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxError, SyntaxOptions};

use super::*;
use crate::core::document::DocumentId;

struct MockEngine {
	parse_count: AtomicUsize,
	result: Arc<parking_lot::Mutex<std::result::Result<Syntax, String>>>,
}

impl MockEngine {
	fn new() -> Self {
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
		}
	}

	fn set_result(&self, res: std::result::Result<Syntax, String>) {
		*self.result.lock() = res;
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

#[tokio::test]
async fn test_single_flight_per_doc() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let ctx = EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	};

	// First poll: kicks task
	let r1 = mgr.ensure_syntax(ctx);
	assert_eq!(r1.result, SyntaxPollResult::Kicked);

	// Second poll (immediate, before task finishes): no duplicate spawn
	let ctx2 = EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	};
	let r2 = mgr.ensure_syntax(ctx2);
	assert_eq!(r2.result, SyntaxPollResult::Pending);

	// Let blocking task complete and verify only one parse was called
	sleep(Duration::from_millis(50)).await;
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_abort_releases_permit() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_rust = loader.language_for_name("rust").unwrap();
	let lang_py = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// Kick task for Doc 1
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang_rust),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(mgr.pending_count(), 1);

	// Switch language for Doc 1 -> aborts old task, kicks new one
	// (the new task re-acquires the permit, proving the old one was released)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang_py),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);

	// Wait for Python task to complete, then drain to release permit
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	// Now kick task for Doc 2 - should work because permit was released
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: DocumentId(2),
		doc_version: 1,
		language_id: Some(lang_rust),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_drain_releases_permit_without_repoll() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	// Wait for task
	sleep(Duration::from_millis(50)).await;

	// Drain - releases permit
	assert!(mgr.drain_finished_inflight());
	assert_eq!(mgr.pending_count(), 0);

	// Kick Doc 2
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: DocumentId(2),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_bootstrap_parse_skips_debounce() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_secs(60); // Long debounce
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Bootstrap poll (no tree) should ignore debounce
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_note_edit_updates_timestamp() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(100);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Establish initial tree
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_syntax(doc_id));

	// 2. Note edit
	mgr.note_edit(doc_id);
	assert!(mgr.is_dirty(doc_id));

	// 3. Poll immediately - should be Pending (debounced)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Pending);

	// 4. Wait for debounce
	sleep(Duration::from_millis(150)).await;
	let r2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r2.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_opts_mismatch_aborts_inflight() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.injections = InjectionPolicy::Eager;
	mgr.set_policy(policy.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// Kick task with Eager injections
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	// Change policy to Disabled injections
	policy.s.injections = InjectionPolicy::Disabled;
	mgr.set_policy(policy);

	// Poll - should abort and release permit for new task
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_opts_mismatch_never_installs() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.injections = InjectionPolicy::Eager;
	mgr.set_policy(policy.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Kick task with Eager injections
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	// 2. Change policy to Disabled injections BEFORE draining
	policy.s.injections = InjectionPolicy::Disabled;
	mgr.set_policy(policy);

	// 3. Wait and drain - result should be discarded because opts key mismatch
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	// Should kick a new task because the old one was discarded
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	// Let blocking task run before checking count
	sleep(Duration::from_millis(50)).await;
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_language_switch_discards_old_parse() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_rust = loader.language_for_name("rust").unwrap();
	let lang_py = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	// 1. Kick Rust parse
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_rust),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	// 2. Switch to Python - aborts Rust inflight, kicks Python immediately
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_py),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);

	// 3. Wait and drain - Python result is ready
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_py),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	// Python result installed; old Rust parse was never installed
	assert_eq!(r.result, SyntaxPollResult::Ready);
	assert!(mgr.has_syntax(doc_id));
}

#[tokio::test]
async fn test_stale_parse_does_not_overwrite_clean_incremental() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Establish initial tree at V1
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));

	// 2. Kick background reparse at V1 (stale)
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	// 3. Complete an interactive edit -> Sync incremental catchup to V2
	// Tree is now clean at V2.
	mgr.note_edit_incremental(
		doc_id,
		2,
		&content,
		&content,
		&ChangeSet::new(content.slice(..)),
		&loader,
	);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(2));
	assert!(!mgr.is_dirty(doc_id));

	// 4. Stale V1 reparse completes
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	// 5. Poll ensure_syntax - V1 result must NOT overwrite V2 tree because dirty=false
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(
		mgr.syntax_doc_version(doc_id),
		Some(2),
		"Stale V1 must not overwrite clean V2"
	);
}

#[tokio::test]
async fn test_stale_install_continuity() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Kick parse at V1
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	// 2. Edit to V2 while parse is inflight
	mgr.note_edit(doc_id);

	// 3. Complete V1 parse
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	// 4. Poll - should install V1 even if stale (V1 != V2) because slot is dirty
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(1));
	assert!(
		mgr.is_dirty(doc_id),
		"Should remain dirty for catch-up reparse"
	);
}

#[tokio::test]
async fn test_dropwhenhidden_discards_completed_parse() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Kick parse while Visible
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	// 2. Drop to Cold
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
	});

	// 3. Complete parse
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	// 4. Poll Cold - should discard result and remain empty
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
	});
	assert!(!mgr.has_syntax(doc_id));
	assert!(!mgr.is_dirty(doc_id), "Cold drop should clear dirty flag");
}

#[tokio::test]
async fn test_inflight_drained_even_if_doc_marked_clean() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	// Artificially clear dirty flag (e.g. by undoing all edits)
	// and simulate the task finishing.
	mgr.entry_mut(doc_id).slot.dirty = false;
	sleep(Duration::from_millis(50)).await;

	// drain should still move result to completed and eventually install it
	mgr.drain_finished_inflight();
	assert!(!mgr.has_pending(doc_id));

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_syntax(doc_id));
}

#[tokio::test]
async fn test_idle_tick_polls_inflight_parse() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));
	assert!(!mgr.any_task_finished());

	sleep(Duration::from_millis(50)).await;
	assert!(mgr.any_task_finished());

	// Simulation of Editor::tick behavior:
	mgr.drain_finished_inflight();
	assert!(!mgr.has_pending(doc_id));
}

#[tokio::test]
async fn test_syntax_version_bumps_on_install() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let v0 = mgr.syntax_version(doc_id);

	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});

	let v1 = mgr.syntax_version(doc_id);
	assert!(v1 > v0);
}

#[tokio::test]
async fn test_language_switch_clears_completed_error() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.cooldown_on_timeout = Duration::from_secs(60);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id_old = loader.language_for_name("rust").unwrap();
	let lang_id_new = loader.language_for_name("python").unwrap();
	let content = Rope::from("test");

	engine.set_result(Err("timeout".to_string()));
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_old),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_old),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::CoolingDown);

	let poll_new = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_new),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll_new.result, SyntaxPollResult::Kicked);
}

/// Verifies that a completed background parse never regresses the installed tree version.
#[tokio::test]
async fn test_monotonic_version_guard() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Establish syntax at V5
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 5,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 5,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(5));

	// 2. Kick another parse at V5 (simulating a slow redundant one)
	mgr.mark_dirty(doc_id);
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 5,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	// 3. Advance to V7 via sync incremental updates
	mgr.note_edit_incremental(
		doc_id,
		6,
		&content,
		&content,
		&ChangeSet::new(content.slice(..)),
		&loader,
	);
	mgr.note_edit_incremental(
		doc_id,
		7,
		&content,
		&content,
		&ChangeSet::new(content.slice(..)),
		&loader,
	);
	assert_eq!(mgr.syntax_doc_version(doc_id), Some(7));

	// 4. Complete the V5 parse
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();

	// 5. Poll and ensure V5 did NOT clobber V7
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 10,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(
		mgr.syntax_doc_version(doc_id),
		Some(7),
		"V5 should not clobber V7"
	);
}

/// Verifies that a document evicted due to Cold hotness is re-bootstrapped
/// immediately when it becomes visible again.
#[tokio::test]
async fn test_cold_eviction_reload() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Establish syntax
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	sleep(Duration::from_millis(50)).await;
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_syntax(doc_id));

	// 2. Trigger eviction
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
	});
	assert!(!mgr.has_syntax(doc_id));

	// 3. Become visible again - should Kick bootstrap immediately
	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
}

/// Verifies that Cold hotness + DropWhenHidden aborts an inflight parse and releases its permit.
#[tokio::test]
async fn test_cold_aborts_inflight() {
	let engine = Arc::new(MockEngine::new());
	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden = RetentionPolicy::DropWhenHidden;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	// 1. Start inflight parse
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert!(mgr.has_pending(doc_id));

	// 2. Drop hotness to Cold - should abort and release permit
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
	});
	assert!(!mgr.has_pending(doc_id));

	// 3. Verify permit released by starting another task
	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id: DocumentId(2),
		doc_version: 1,
		language_id: Some(lang_id),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
	});
	assert_eq!(poll.result, SyntaxPollResult::Kicked);
}
