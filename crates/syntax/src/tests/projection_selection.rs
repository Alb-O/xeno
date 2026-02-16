use super::*;

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
		let loader = Arc::new(LanguageLoader::from_embedded());
		let lang = loader.language_for_name("rust").unwrap();
		let entry = mgr.entry_mut(doc_id);
		let syntax = Syntax::new(old_rope.slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
		entry.slot.full = Some(InstalledTree {
			syntax,
			doc_version: 1,
			tree_id: 0,
		});
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
fn test_selection_prefers_full_tree_over_eager_viewport() {
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
		SyntaxOptions {
			injections: InjectionPolicy::Disabled,
			..Default::default()
		},
	)
	.unwrap();

	// Install eager viewport tree
	let eager_tree = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions {
			injections: InjectionPolicy::Eager,
			..Default::default()
		},
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(InstalledTree {
			syntax: full_tree,
			doc_version: 1,
			tree_id: tid,
		});

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

	// Full tree should be preferred over eager viewport for complete
	// structural context (e.g. file-spanning block comments).
	let sel = mgr.syntax_for_viewport(doc_id, 1, 0..10).unwrap();
	assert_eq!(sel.syntax.opts().injections, InjectionPolicy::Disabled);
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
		SyntaxOptions {
			injections: InjectionPolicy::Disabled,
			..Default::default()
		},
	)
	.unwrap();

	{
		let entry = mgr.entry_mut(doc_id);
		let tid = entry.slot.alloc_tree_id();
		entry.slot.full = Some(InstalledTree {
			syntax: full_tree,
			doc_version: 1,
			tree_id: tid,
		});
		entry.slot.dirty = false;
		entry.slot.language_id = Some(lang);
		entry.slot.last_opts_key = Some(OptKey {
			injections: InjectionPolicy::Disabled,
		});
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
		SyntaxOptions {
			injections: InjectionPolicy::Disabled,
			..Default::default()
		},
	)
	.unwrap();

	let tree_b = Syntax::new(
		content.slice(..),
		lang,
		&loader,
		SyntaxOptions {
			injections: InjectionPolicy::Disabled,
			..Default::default()
		},
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
