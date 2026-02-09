use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::sleep;
use xeno_primitives::transaction::Change;
use xeno_primitives::{Rope, Transaction};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::InjectionPolicy;

use super::invariants::{EngineGuard, MockEngine};
use super::*;
use crate::core::document::DocumentId;

#[tokio::test]
async fn test_drain_releases_permit_without_repoll() {
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
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
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
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
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
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 1,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
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
async fn test_viewport_stage_b_budget_gate() {
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
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_millis(100));
	mgr.set_policy(policy.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// 1. Kick Stage A
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	engine.proceed();
	while !mgr.any_task_finished() {
		sleep(Duration::from_millis(1)).await;
	}
	mgr.drain_finished_inflight();

	// Poll - installs Stage A and should kick Full parse (skipping Stage B due to budget)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));
}

#[tokio::test]
async fn test_viewport_policy_flip_discard() {
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
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_millis(100));
	mgr.set_policy(policy.clone());

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// 1. Install Stage A
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	engine.proceed();
	while !mgr.any_task_finished() {
		sleep(Duration::from_millis(1)).await;
	}
	mgr.drain_finished_inflight();
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});

	// 2. Kick Stage B
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	assert!(mgr.has_pending(doc_id));

	// 3. Disable Stage B policy BEFORE draining
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	// 4. Complete Stage B
	engine.proceed();
	while !mgr.any_task_finished() {
		sleep(Duration::from_millis(1)).await;
	}
	mgr.drain_finished_inflight();

	// Poll - should NOT install Stage B because it no longer matches policy
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..10),
	});
	assert_eq!(
		mgr.syntax_for_doc(doc_id).unwrap().opts().injections,
		InjectionPolicy::Disabled
	);
}

#[test]
fn test_highlight_projection_ctx_available_for_aligned_pending_window() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);

	let old_rope = Rope::from("abcdef");
	let tx = Transaction::change(
		old_rope.slice(..),
		[Change {
			start: 0,
			end: 1,
			replacement: None,
		}],
	);

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.tree_doc_version = Some(1);
		entry.slot.pending_incremental = Some(PendingIncrementalEdits {
			base_tree_doc_version: 1,
			old_rope: old_rope.clone(),
			composed: tx.changes().clone(),
		});
	}

	assert!(mgr.highlight_projection_ctx(doc_id, 2).is_some());
	assert!(mgr.highlight_projection_ctx(doc_id, 1).is_none());
}
