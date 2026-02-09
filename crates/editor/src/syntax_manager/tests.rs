use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::sleep;
use xeno_primitives::Rope;
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::InjectionPolicy;

use super::invariants::{EngineGuard, MockEngine};
use super::*;
use crate::core::document::DocumentId;

#[tokio::test]
async fn test_drain_releases_permit_without_repoll() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
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
		viewport: None,
	});

	// Wait for task
	engine.proceed();

	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished());

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
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_opts_mismatch_invalidates_and_throttles() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
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
		viewport: None,
	});
	assert!(mgr.has_pending(doc_id));

	// Change policy to Disabled injections
	policy.s.injections = InjectionPolicy::Disabled;
	mgr.set_policy(policy);

	// Poll - should invalidate and return Throttled (permit still held)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Throttled);

	// Finish task, result will be discarded due to epoch mismatch
	engine.proceed();

	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished());

	mgr.drain_finished_inflight();

	// Next poll should kick
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_opts_mismatch_never_installs() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
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
		viewport: None,
	});

	// 2. Change policy to Disabled injections BEFORE draining
	policy.s.injections = InjectionPolicy::Disabled;
	mgr.set_policy(policy);

	// 3. Wait and drain - result should be discarded because epoch mismatch (invalidated on opts change)
	engine.proceed();

	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished());

	mgr.drain_finished_inflight();

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	// Should kick a new task because the old one was discarded
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	// Let blocking task run before checking count
	engine.proceed();

	iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished());

	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_dropwhenhidden_discards_completed_parse() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
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
		viewport: None,
	});

	// 2. Drop to Cold
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});

	// 3. Complete parse
	engine.proceed();

	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished());

	mgr.drain_finished_inflight();

	// 4. Poll Cold - should discard result and remain empty
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert!(!mgr.has_syntax(doc_id));
	assert!(!mgr.is_dirty(doc_id), "Cold drop should clear dirty flag");
}

#[tokio::test]
async fn test_language_switch_clears_completed_error() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
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
		viewport: None,
	});
	engine.proceed();

	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert!(mgr.any_task_finished());

	mgr.drain_finished_inflight();

	let poll = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_old),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(poll.result, SyntaxPollResult::CoolingDown);

	let poll_new = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang_id_new),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(poll_new.result, SyntaxPollResult::Kicked);
}
