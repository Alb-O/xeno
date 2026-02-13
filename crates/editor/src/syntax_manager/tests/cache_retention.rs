use super::*;

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
	ce1.stage_a = Some(ViewportTree {
		syntax: syntax.clone(),
		doc_version: 1,
		tree_id: 1,
		coverage: 0..50000,
	});
	let ce2 = cache.get_mut_or_insert(k2);
	ce2.stage_a = Some(ViewportTree {
		syntax: syntax.clone(),
		doc_version: 1,
		tree_id: 2,
		coverage: 0..50000,
	});

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
		ce1.stage_a = Some(ViewportTree {
			syntax: syntax.clone(),
			doc_version: 1,
			tree_id: 10,
			coverage: 0..50000,
		});

		let ce2 = entry.slot.viewport_cache.get_mut_or_insert(k2);
		ce2.stage_a = Some(ViewportTree {
			syntax: syntax.clone(),
			doc_version: 1,
			tree_id: 20,
			coverage: 0..50000,
		});
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
	let mut mgr = SyntaxManager::new_with_engine(SyntaxManagerCfg::default(), engine.clone());
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
	mgr.ensure_syntax_at(
		t0,
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
			viewport: None,
		},
	);

	// Seed a viewport cache entry
	{
		let entry = mgr.entry_mut(doc_id);
		let tree = Syntax::new(
			content.slice(..),
			lang,
			&loader,
			SyntaxOptions {
				injections: InjectionPolicy::Disabled,
				..Default::default()
			},
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
	mgr.ensure_syntax_at(
		t0 + Duration::from_secs(30),
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang),
			content: &content,
			hotness: SyntaxHotness::Cold,
			loader: &loader,
			viewport: None,
		},
	);
	assert!(mgr.entry_mut(doc_id).slot.viewport_cache.has_any(), "viewport should survive within TTL");

	// Poll as Cold at t0 + 61s → past TTL, viewport should be dropped
	mgr.ensure_syntax_at(
		t0 + Duration::from_secs(61),
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang),
			content: &content,
			hotness: SyntaxHotness::Cold,
			loader: &loader,
			viewport: None,
		},
	);
	assert!(!mgr.entry_mut(doc_id).slot.viewport_cache.has_any(), "viewport should be dropped after TTL");
}

/// Deterministic Stage-B per-key cooldown: blocks re-kick until cooldown
/// expires, then allows retry.
#[tokio::test]
async fn test_stage_b_cooldown_blocks_then_allows_retry() {
	use xeno_language::syntax::SyntaxError;

	use super::scheduling::{CompletedSyntaxTask, ViewportLane};

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
				SyntaxOptions {
					injections: InjectionPolicy::Disabled,
					..Default::default()
				},
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
			opts: super::types::OptKey {
				injections: InjectionPolicy::Eager,
			},
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
	use xeno_language::syntax::SyntaxError;

	use super::scheduling::CompletedSyntaxTask;

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
			opts: super::types::OptKey {
				injections: InjectionPolicy::Eager,
			},
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
