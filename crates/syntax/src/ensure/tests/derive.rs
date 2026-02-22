use super::*;

#[test]
fn derive_viewport_clamps_to_doc_length() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello"); // 5 bytes
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, Some(0..1000), SyntaxHotness::Visible);
	let base = derive::derive(&ctx, &policy);

	assert!(base.viewport.is_some());
	let vp = base.viewport.unwrap();
	assert_eq!(vp.end, 5, "viewport end must clamp to doc length");
}

#[test]
fn derive_viewport_caps_span() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	// Create content larger than viewport_visible_span_cap (default 64KB for S).
	let content = Rope::from("x".repeat(200_000));
	let policy = TieredSyntaxPolicy::default();
	let cap = policy.s.viewport_visible_span_cap;

	let ctx = make_derive_ctx(&content, &loader, Some(0..200_000), SyntaxHotness::Visible);
	let base = derive::derive(&ctx, &policy);

	let vp = base.viewport.unwrap();
	assert_eq!(vp.end - vp.start, cap, "viewport span must be capped to visible_span_cap");
}

#[test]
fn derive_viewport_handles_reversed_range() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello world");
	let policy = TieredSyntaxPolicy::default();

	#[allow(clippy::reversed_empty_ranges)]
	let reversed_viewport = Some(8..3);
	let ctx = make_derive_ctx(&content, &loader, reversed_viewport, SyntaxHotness::Visible);
	let base = derive::derive(&ctx, &policy);

	let vp = base.viewport.unwrap();
	assert!(vp.start <= vp.end, "reversed range must be normalized: start={}, end={}", vp.start, vp.end);
}

#[test]
fn derive_cold_hidden_disables_work() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello");
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, None, SyntaxHotness::Cold);
	let base = derive::derive(&ctx, &policy);

	assert!(base.work_disabled, "cold + parse_when_hidden=false must disable work");
}

#[test]
fn derive_warm_hidden_allows_work() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let content = Rope::from("hello");
	let policy = TieredSyntaxPolicy::default();

	let ctx = make_derive_ctx(&content, &loader, None, SyntaxHotness::Warm);
	let base = derive::derive(&ctx, &policy);

	assert!(!base.work_disabled, "warm docs must allow work even with parse_when_hidden=false");
}
