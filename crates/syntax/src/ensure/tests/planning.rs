use super::*;

#[test]
fn test_compute_viewport_key_aligns_to_half_window_stride() {
	let key = compute_viewport_key(70_000, 131_072);
	assert_eq!(key, ViewportKey(65_536));
}

#[test]
fn test_compute_viewport_key_respects_min_stride_floor() {
	let key = compute_viewport_key(9_000, 4_096);
	assert_eq!(key, ViewportKey(8_192));
}

/// When Stage-A is planned, Stage-B must not be planned (avoids concurrent
/// viewport parsing contention).
#[test]
fn test_plan_stage_a_suppresses_stage_b() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut entry = DocEntry::new(Instant::now());
	entry.slot.language_id = Some(lang);
	// No full tree → Stage-A will be planned for viewport uncovered.

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p.l.viewport_stage_b_budget = Some(Duration::from_secs(10));
		p.l.viewport_stage_b_min_stable_polls = 0;
		p
	};

	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	};

	let d = derive(&ctx, &policy);
	let lang_ctx = d.clone().into_lang(lang);
	let g = GateState {
		viewport_stable_polls: 10, // well above min
		viewport_uncovered: true,
	};

	let plan = compute_plan(&entry, Instant::now(), &lang_ctx, &g, &SyntaxMetrics::new());
	assert!(plan.stage_a.is_some(), "Stage-A should be planned (viewport uncovered)");
	assert!(plan.stage_b.is_none(), "Stage-B must not be planned when Stage-A is planned");
}

/// BG can be planned independently of Stage-A.
#[test]
fn test_plan_bg_independent_of_stage_a() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("x".repeat(300_000));

	let mut entry = DocEntry::new(Instant::now());
	entry.slot.language_id = Some(lang);
	entry.slot.dirty = true;
	// No full tree → both Stage-A and BG should plan.

	let policy = {
		let mut p = TieredSyntaxPolicy::test_default();
		p.s_max_bytes_inclusive = 0;
		p.m_max_bytes_inclusive = 0;
		p.l.debounce = Duration::ZERO;
		p.l.viewport_stage_b_budget = None;
		p
	};

	let ctx = EnsureSyntaxContext {
		doc_id: DocumentId(1),
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: Some(0..100),
	};

	let d = derive(&ctx, &policy);
	let lang_ctx = d.clone().into_lang(lang);
	let g = GateState::default();

	let plan = compute_plan(&entry, Instant::now(), &lang_ctx, &g, &SyntaxMetrics::new());
	assert!(plan.stage_a.is_some(), "Stage-A should be planned");
	assert!(plan.bg.is_some(), "BG should be planned independently of Stage-A");
}
