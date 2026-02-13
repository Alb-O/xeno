use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::sleep;
use xeno_language::LanguageLoader;
use xeno_language::syntax::{InjectionPolicy, Syntax, SyntaxOptions};
use xeno_primitives::transaction::Change;
use xeno_primitives::{Rope, Transaction};

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

	// Poll - should invalidate and return Pending (permit still held, work desired)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Pending);

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
	policy.s.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.s.retention_hidden_viewport = RetentionPolicy::DropWhenHidden;
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
	assert_eq!(mgr.syntax_for_doc(doc_id).unwrap().opts().injections, InjectionPolicy::Disabled);
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
		entry.slot.full_doc_version = Some(1);
		entry.slot.pending_incremental = Some(PendingIncrementalEdits {
			base_tree_doc_version: 1,
			old_rope: old_rope.clone(),
			composed: tx.changes().clone(),
		});
	}

	assert!(mgr.highlight_projection_ctx(doc_id, 2).is_some());
	assert!(mgr.highlight_projection_ctx(doc_id, 1).is_none());
}

#[test]
fn test_selection_prefers_eager_viewport_over_disabled_full() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// Install full tree with Disabled injections
	let full_tree = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
	)
	.unwrap();

	// Install eager viewport tree
	let eager_tree = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions { injections: InjectionPolicy::Eager, ..Default::default() },
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(full_tree);
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tid;

		let tid2 = entry.slot.alloc_tree_id();
		let coverage = 0..content.len_bytes() as u32;
		let vp_key = ViewportKey(0);
		let cache_entry = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
		cache_entry.stage_b = Some(ViewportTree {
			syntax: eager_tree,
			doc_version: 1,
			tree_id: tid2,
			coverage,
		});
	}

	// Selection for overlapping viewport should prefer eager viewport
	let sel = mgr.syntax_for_viewport(doc_id, 1, 0..10).unwrap();
	assert_eq!(sel.syntax.opts().injections, InjectionPolicy::Eager);
}

#[tokio::test]
async fn test_enrichment_schedules_stage_b_when_full_exists_and_clean() {
	let engine = Arc::new(MockEngine::new());
	let _guard = EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine.clone(),
	);

	// Configure as tier L by setting thresholds low so our content falls into L
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_millis(500));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	// Content just needs to exist; tier is determined by policy thresholds
	let content = Rope::from("fn main() { let x = 1; }");

	// Install a full tree directly (simulating a completed full parse)
	let full_tree = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(full_tree);
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tid;
		entry.slot.dirty = false;
		entry.slot.language_id = Some(lang);
		entry.slot.last_opts_key = Some(OptKey { injections: InjectionPolicy::Disabled });
	}

	// Poll with viewport — should schedule Stage-B enrichment even though not dirty
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..20),
	});

	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport(doc_id));
}

#[test]
fn test_viewport_cache_selects_overlapping_entry() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	let tree_a = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
	)
	.unwrap();

	let tree_b = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);

		// Entry at key 0, covering bytes 0..50
		let tid_a = entry.slot.alloc_tree_id();
		let ce_a = entry.slot.viewport_cache.get_mut_or_insert(ViewportKey(0));
		ce_a.stage_a = Some(ViewportTree {
			syntax: tree_a,
			doc_version: 1,
			tree_id: tid_a,
			coverage: 0..50,
		});

		// Entry at key 100, covering bytes 100..200
		let tid_b = entry.slot.alloc_tree_id();
		let ce_b = entry.slot.viewport_cache.get_mut_or_insert(ViewportKey(100));
		ce_b.stage_a = Some(ViewportTree {
			syntax: tree_b,
			doc_version: 1,
			tree_id: tid_b,
			coverage: 100..200,
		});
	}

	// Query for viewport in 0..10 → should pick entry at key 0
	let sel = mgr.syntax_for_viewport(doc_id, 1, 0..10).unwrap();
	assert_eq!(sel.coverage, Some(0..50));

	// Query for viewport in 110..150 → should pick entry at key 100
	let sel = mgr.syntax_for_viewport(doc_id, 1, 110..150).unwrap();
	assert_eq!(sel.coverage, Some(100..200));

	// Query for viewport in 60..90 → no overlap, should still return best-effort
	let sel = mgr.syntax_for_viewport(doc_id, 1, 60..90);
	assert!(sel.is_some());
}

#[tokio::test]
async fn test_stage_b_is_tracked_per_key() {
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
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_millis(500));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() { let x = 1; }");

	// Install a full tree so Stage-A is not needed
	let full_tree = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(full_tree);
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tid;
		entry.slot.dirty = false;
		entry.slot.language_id = Some(lang);
		entry.slot.last_opts_key = Some(OptKey { injections: InjectionPolicy::Disabled });
	}

	// First viewport → should kick Stage-B enrichment
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..20),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);

	// Complete the task
	engine.proceed();
	let mut iters = 0;
	while !mgr.any_task_finished() && iters < 100 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	mgr.drain_finished_inflight();

	// Same viewport key again → should NOT kick (attempted_b_for is set)
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..20),
	});
	// Should be Ready since we have full tree + enrichment was attempted
	assert_ne!(r.result, SyntaxPollResult::Kicked);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_viewport_kicks_while_bg_running() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	// Force tier L so viewport scheduling activates
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// First ensure: should kick viewport (Stage-A) since no tree exists
	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_eq!(r1.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport(doc_id), "viewport lane should be active");
	assert!(mgr.has_inflight_bg(doc_id), "bg lane should be active");

	// Bg uses engine.parse (blocks on notify); viewport runs directly via Syntax::new_viewport.
	let mut iters = 0;
	while engine.parse_count.load(Ordering::SeqCst) < 1 && iters < 200 {
		sleep(Duration::from_millis(1)).await;
		iters += 1;
	}
	assert_eq!(engine.parse_count.load(Ordering::SeqCst), 1, "bg should have entered engine");

	// Unblock the bg task
	engine.proceed();
	super::invariants::wait_for_finish(&mgr).await;
	assert!(mgr.drain_finished_inflight());
}

#[tokio::test]
async fn test_stage_a_kicks_when_partial_overlap_only() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	// Seed a viewport tree that overlaps but doesn't fully cover [1000..2000]
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let tree_id = entry.slot.alloc_tree_id();
		let syntax = Syntax::new(
			Rope::from("fn main() {}").slice(..),
			lang,
			&loader,
			SyntaxOptions::default(),
		)
		.unwrap();
		let key = super::types::ViewportKey(0);
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id,
			coverage: 500..1500, // overlaps [1000..2000] but doesn't cover it
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(1000..2000),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id), "Stage-A should kick for partial overlap");
}

#[tokio::test]
async fn test_stage_a_can_kick_while_enrich_lane_active() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	// Pre-seed: enrich lane active, no full tree, viewport uncovered
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.active_viewport_enrich = Some(super::types::TaskId(123));
		entry.sched.active_viewport_enrich_detached = false;
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id), "Stage-A should kick even with enrich active");
}

#[tokio::test]
async fn test_l_retention_drops_full_keeps_viewport_cache() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.l.retention_hidden_viewport = RetentionPolicy::Keep;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed full tree + viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

		let tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(syntax.clone());
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tree_id;

		let vp_tree_id = entry.slot.alloc_tree_id();
		let key = super::types::ViewportKey(0);
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: vp_tree_id,
			coverage: 0..50000,
		});
	}

	// Ensure with Cold → retention should drop full but keep viewport
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Disabled);

	let entry = mgr.entry_mut(doc_id);
	assert!(entry.slot.full.is_none(), "full tree should be dropped");
	assert!(entry.slot.viewport_cache.has_any(), "viewport cache should be kept");
}

#[tokio::test]
async fn test_l_retention_drops_viewport_when_configured() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 2,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.retention_hidden_full = RetentionPolicy::DropWhenHidden;
	policy.l.retention_hidden_viewport = RetentionPolicy::DropWhenHidden;
	policy.l.viewport_stage_b_budget = None;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		let vp_tree_id = entry.slot.alloc_tree_id();
		let key = super::types::ViewportKey(0);
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: vp_tree_id,
			coverage: 0..50000,
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Disabled);

	let entry = mgr.entry_mut(doc_id);
	assert!(!entry.slot.viewport_cache.has_any(), "viewport cache should be dropped");
}

#[tokio::test]
async fn test_stage_b_timeout_sets_per_key_cooldown_and_allows_retry() {
	use super::scheduling::{CompletedSyntaxTask, ViewportLane};
	use xeno_language::syntax::SyntaxError;

	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_cooldown_on_timeout = Duration::ZERO; // instant retry
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed: full tree + viewport cache with stage_a
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		let tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(syntax.clone());
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = tree_id;

		let key = super::types::ViewportKey(0);
		let vp_tree_id = entry.slot.alloc_tree_id();
		let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
		ce.stage_a = Some(super::types::ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: vp_tree_id,
			coverage: 0..50000,
		});

		// Inject Stage-B timeout completion
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: super::types::OptKey { injections: InjectionPolicy::Eager },
			result: Err(SyntaxError::Timeout),
			class: super::tasks::TaskClass::Viewport,
			injections: InjectionPolicy::Eager,
			elapsed: Duration::from_millis(100),
			viewport_key: Some(key),
			viewport_lane: Some(ViewportLane::Enrich),
		});
	}

	// First ensure: should process the timeout but NOT return CoolingDown
	let r1 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_ne!(r1.result, SyntaxPollResult::CoolingDown, "Stage-B timeout should not cause global cooldown");
	// The same ensure call processed the timeout (clearing latch) and immediately re-kicked Stage-B
	assert!(mgr.has_inflight_viewport_enrich(doc_id), "Stage-B should retry immediately with zero cooldown");
}

#[tokio::test]
async fn test_stage_b_failure_does_not_block_stage_a() {
	use super::scheduling::{CompletedSyntaxTask, ViewportLane};
	use xeno_language::syntax::SyntaxError;

	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_cooldown_on_error = Duration::from_secs(9999);
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed: no full tree, inject Stage-B error into completion queue
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: super::types::OptKey { injections: InjectionPolicy::Eager },
			result: Err(SyntaxError::Parse("test error".to_string())),
			class: super::tasks::TaskClass::Viewport,
			injections: InjectionPolicy::Eager,
			elapsed: Duration::from_millis(50),
			viewport_key: Some(super::types::ViewportKey(0)),
			viewport_lane: Some(ViewportLane::Enrich),
		});
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_ne!(r.result, SyntaxPollResult::CoolingDown, "Stage-B error should not cause global cooldown");
	assert_eq!(r.result, SyntaxPollResult::Kicked);
	assert!(mgr.has_inflight_viewport_urgent(doc_id), "Stage-A should still kick despite Stage-B error");
}

#[test]
fn test_covering_key_prefers_mru_entry() {
	use super::types::{ViewportCache, ViewportKey, ViewportTree};

	let mut cache = ViewportCache::new(4);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

	let k1 = ViewportKey(0);
	let k2 = ViewportKey(100);

	// Insert k1 then k2 (k2 is MRU)
	let ce1 = cache.get_mut_or_insert(k1);
	ce1.stage_a = Some(ViewportTree { syntax: syntax.clone(), doc_version: 1, tree_id: 1, coverage: 0..50000 });
	let ce2 = cache.get_mut_or_insert(k2);
	ce2.stage_a = Some(ViewportTree { syntax: syntax.clone(), doc_version: 1, tree_id: 2, coverage: 0..50000 });

	// Both cover [1000..2000], k2 is MRU
	assert_eq!(cache.covering_key(&(1000..2000)), Some(k2));

	// Touch k1 to make it MRU
	cache.touch(k1);
	assert_eq!(cache.covering_key(&(1000..2000)), Some(k1));
}

#[test]
fn test_syntax_for_viewport_tie_breaks_by_mru() {
	use super::types::{ViewportKey, ViewportTree};

	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let syntax = Syntax::new(Rope::from("fn main() {}").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

	let k1 = ViewportKey(0);
	let k2 = ViewportKey(100);

	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);

		// Insert two stage_a trees with identical scores, both overlapping [500..1000]
		let ce1 = entry.slot.viewport_cache.get_mut_or_insert(k1);
		ce1.stage_a = Some(ViewportTree { syntax: syntax.clone(), doc_version: 1, tree_id: 10, coverage: 0..50000 });

		let ce2 = entry.slot.viewport_cache.get_mut_or_insert(k2);
		ce2.stage_a = Some(ViewportTree { syntax: syntax.clone(), doc_version: 1, tree_id: 20, coverage: 0..50000 });
	}

	// k2 is MRU (inserted last), should win ties
	let sel = mgr.syntax_for_viewport(doc_id, 1, 500..1000).unwrap();
	assert_eq!(sel.tree_id, 20, "MRU entry should win tie");

	// Touch k1 to make it MRU
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.viewport_cache.touch(k1);
	}

	let sel = mgr.syntax_for_viewport(doc_id, 1, 500..1000).unwrap();
	assert_eq!(sel.tree_id, 10, "after touch, k1 should win");
}

/// Deterministic DropAfter retention: viewport cache is kept within TTL and
/// dropped after it expires.
#[tokio::test]
async fn test_retention_dropafter_viewport_ttl() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg::default(),
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s.debounce = Duration::ZERO;
	policy.s.retention_hidden_full = RetentionPolicy::Keep;
	policy.s.retention_hidden_viewport = RetentionPolicy::DropAfter(Duration::from_secs(60));
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	let t0 = Instant::now();

	// Poll as Visible to set last_visible_at = t0
	mgr.ensure_syntax_at(t0, EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});

	// Seed a viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		let tree = Syntax::new(
			content.slice(..),
			lang,
			&loader,
			SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
		)
		.unwrap();
		let tid = entry.slot.alloc_tree_id();
		let ce = entry.slot.viewport_cache.get_mut_or_insert(ViewportKey(0));
		ce.stage_a = Some(ViewportTree {
			syntax: tree,
			doc_version: 1,
			tree_id: tid,
			coverage: 0..content.len_bytes() as u32,
		});
	}
	assert!(mgr.entry_mut(doc_id).slot.viewport_cache.has_any());

	// Poll as Cold at t0 + 30s → within TTL, viewport should survive
	mgr.ensure_syntax_at(t0 + Duration::from_secs(30), EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert!(mgr.entry_mut(doc_id).slot.viewport_cache.has_any(), "viewport should survive within TTL");

	// Poll as Cold at t0 + 61s → past TTL, viewport should be dropped
	mgr.ensure_syntax_at(t0 + Duration::from_secs(61), EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Cold,
		loader: &loader,
		viewport: None,
	});
	assert!(!mgr.entry_mut(doc_id).slot.viewport_cache.has_any(), "viewport should be dropped after TTL");
}

/// Deterministic Stage-B per-key cooldown: blocks re-kick until cooldown
/// expires, then allows retry.
#[tokio::test]
async fn test_stage_b_cooldown_blocks_then_allows_retry() {
	use super::scheduling::{CompletedSyntaxTask, ViewportLane};
	use xeno_language::syntax::SyntaxError;

	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 1;
	policy.l.viewport_cooldown_on_timeout = Duration::from_secs(5);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));
	let t0 = Instant::now();

	// Seed a full tree
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Inject a Stage-B timeout completion at t0
	let key = ViewportKey(0);
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.viewport_cache.get_mut_or_insert(key);
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: super::types::OptKey { injections: InjectionPolicy::Eager },
			result: Err(SyntaxError::Timeout),
			class: super::tasks::TaskClass::Viewport,
			injections: InjectionPolicy::Eager,
			elapsed: Duration::from_millis(100),
			viewport_key: Some(key),
			viewport_lane: Some(ViewportLane::Enrich),
		});
	}

	let ctx = || EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	};

	// ensure at t0 → processes timeout, sets 5s cooldown, should NOT kick enrich
	mgr.ensure_syntax_at(t0, ctx());
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "should not kick during cooldown");

	// ensure at t0 + 4s → still in cooldown
	mgr.ensure_syntax_at(t0 + Duration::from_secs(4), ctx());
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "still in cooldown at t0+4s");

	// ensure at t0 + 6s → cooldown expired, should kick enrich
	mgr.ensure_syntax_at(t0 + Duration::from_secs(6), ctx());
	assert!(mgr.has_inflight_viewport_enrich(doc_id), "should kick after cooldown expires");
}

/// Deterministic doc-level cooldown: blocks background tasks until cooldown
/// expires.
#[tokio::test]
async fn test_doc_cooldown_blocks_bg_tasks() {
	use super::scheduling::{CompletedSyntaxTask, ViewportLane};
	use xeno_language::syntax::SyntaxError;

	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s.debounce = Duration::ZERO;
	policy.s.cooldown_on_error = Duration::from_secs(10);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");
	let t0 = Instant::now();

	// Inject a bg error completion at t0
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.language_id = Some(lang);
		entry.sched.completed.push_back(CompletedSyntaxTask {
			doc_version: 1,
			lang_id: lang,
			opts: super::types::OptKey { injections: InjectionPolicy::Eager },
			result: Err(SyntaxError::Parse(String::from("test error"))),
			class: super::tasks::TaskClass::Full,
			injections: InjectionPolicy::Eager,
			elapsed: Duration::from_millis(50),
			viewport_key: None,
			viewport_lane: None,
		});
	}

	let ctx = || EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	};

	// ensure at t0 → processes error, returns CoolingDown
	let r = mgr.ensure_syntax_at(t0, ctx());
	assert_eq!(r.result, SyntaxPollResult::CoolingDown);

	// ensure at t0 + 5s → still CoolingDown
	let r = mgr.ensure_syntax_at(t0 + Duration::from_secs(5), ctx());
	assert_eq!(r.result, SyntaxPollResult::CoolingDown);

	// ensure at t0 + 11s → cooldown expired, should kick
	let r = mgr.ensure_syntax_at(t0 + Duration::from_secs(11), ctx());
	assert_eq!(r.result, SyntaxPollResult::Kicked);
}

/// Stage-B must not kick until the viewport focus has been polled at least
/// `viewport_stage_b_min_stable_polls` times on the same key+version.
#[tokio::test]
async fn test_stage_b_requires_stable_polls() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 3;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Bootstrap with a full tree so Stage-B has something to enrich.
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Poll 1 — should NOT kick Stage-B (1 < 3)
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "poll 1: not enough stable polls");

	// Poll 2 — still not enough (2 < 3)
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert!(!mgr.has_inflight_viewport_enrich(doc_id), "poll 2: not enough stable polls");

	// Poll 3 — now it should fire (3 >= 3)
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert!(mgr.has_inflight_viewport_enrich(doc_id), "poll 3: should kick Stage-B");
}

/// Stage-B stability gating must track the covering enrichment key, not just
/// the computed viewport anchor key.
#[tokio::test]
async fn test_stage_b_stability_uses_covering_key_across_stride_boundary() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_window_max = 65_536;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 2;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed a full tree and a wide stage_a cache entry at ViewportKey(0) that
	// covers both viewports, even though their computed anchor keys differ.
	{
		let entry = mgr.entry_mut(doc_id);
		let syntax = Syntax::new(
			content.slice(..),
			lang,
			&loader,
			SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
		)
		.unwrap();
		let full_tree_id = entry.slot.alloc_tree_id();
		entry.slot.full = Some(syntax.clone());
		entry.slot.full_doc_version = Some(1);
		entry.slot.full_tree_id = full_tree_id;
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;

		let stage_a_tree_id = entry.slot.alloc_tree_id();
		let cache_entry = entry.slot.viewport_cache.get_mut_or_insert(ViewportKey(0));
		cache_entry.stage_a = Some(ViewportTree {
			syntax,
			doc_version: 1,
			tree_id: stage_a_tree_id,
			coverage: 0..200_000,
		});
	}

	// Poll 1: stable polls = 1, should not kick Stage-B yet.
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(65_000..65_100),
	});
	assert!(
		!mgr.has_inflight_viewport_enrich(doc_id),
		"poll 1: not enough stable polls"
	);

	// Poll 2: viewport crosses stride boundary, but covering key remains 0, so
	// Stage-B should now kick at stable polls = 2.
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(70_000..70_100),
	});
	assert!(
		mgr.has_inflight_viewport_enrich(doc_id),
		"poll 2: should kick Stage-B when covering key is stable"
	);
}

/// When the viewport key flips every poll (fast scrolling), the stable poll
/// counter resets and Stage-B never fires.
#[tokio::test]
async fn test_stage_b_does_not_kick_on_scroll_key_flip() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 2;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Bootstrap with a full tree.
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Alternate viewport positions so the key keeps changing
	let viewports = [0..100, 200_000..200_100, 0..100, 200_000..200_100];
	for vp in &viewports {
		mgr.ensure_syntax(EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
			viewport: Some(vp.clone()),
		});
		assert!(
			!mgr.has_inflight_viewport_enrich(doc_id),
			"Stage-B should not kick during fast scrolling (viewport key keeps flipping)"
		);
	}
}

/// When Stage-B enrichment is desired but deferred (stability gate not yet
/// met), the poll result must be Pending, not Ready.
#[tokio::test]
async fn test_stage_b_deferral_returns_pending_not_throttled() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg {
			max_concurrency: 4,
			..Default::default()
		},
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s_max_bytes_inclusive = 0;
	policy.m_max_bytes_inclusive = 0;
	policy.l.debounce = Duration::ZERO;
	policy.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
	policy.l.viewport_stage_b_min_stable_polls = 3;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	// Seed full tree so dirty=false and full exists
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions { injections: InjectionPolicy::Disabled, ..Default::default() },
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
	}

	// Poll once — stability gate not met (1 < 3), should return Pending
	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	});
	assert_eq!(r.result, SyntaxPollResult::Pending, "enrich desired but deferred → Pending");
	assert!(!mgr.has_inflight_viewport_enrich(doc_id));
}

/// When the document is fully parsed at the current version with no pending
/// work, the poll result must be Ready.
#[tokio::test]
async fn test_no_work_returns_ready() {
	let engine = Arc::new(super::invariants::MockEngine::new());
	let _guard = super::invariants::EngineGuard(engine.clone());
	let mut mgr = SyntaxManager::new_with_engine(
		SyntaxManagerCfg::default(),
		engine.clone(),
	);
	let mut policy = TieredSyntaxPolicy::test_default();
	policy.s.debounce = Duration::ZERO;
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");

	// Seed full tree at exact version, not dirty
	{
		let entry = mgr.entry_mut(doc_id);
		entry.slot.full = Some(
			Syntax::new(
				content.slice(..),
				lang,
				&loader,
				SyntaxOptions { injections: InjectionPolicy::Eager, ..Default::default() },
			)
			.unwrap(),
		);
		entry.slot.full_doc_version = Some(1);
		entry.slot.language_id = Some(lang);
		entry.slot.dirty = false;
		entry.slot.last_opts_key = Some(super::types::OptKey { injections: InjectionPolicy::Eager });
	}

	let r = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(r.result, SyntaxPollResult::Ready, "fully parsed, no work → Ready");
}
