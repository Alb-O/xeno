use xeno_lsp::lsp_types::CompletionItem;

use super::*;

fn make_item(label: &str) -> CompletionItem {
	CompletionItem {
		label: label.to_string(),
		..Default::default()
	}
}

#[test]
fn empty_query_returns_all() {
	let items = vec![make_item("foo"), make_item("bar"), make_item("baz")];
	let filtered = filter_items(&items, "");
	assert_eq!(filtered.len(), 3);
	assert_eq!(filtered[0].index, 0);
	assert_eq!(filtered[1].index, 1);
	assert_eq!(filtered[2].index, 2);
	// Empty query has no match indices
	assert!(filtered[0].match_indices.is_none());
}

#[test]
fn filters_by_prefix() {
	let items = vec![make_item("tracing"), make_item("error"), make_item("tree"), make_item("result")];
	let filtered = filter_items(&items, "tr");
	assert_eq!(filtered.len(), 2);
	let labels: Vec<_> = filtered.iter().map(|f| items[f.index].label.as_str()).collect();
	assert!(labels.contains(&"tracing"));
	assert!(labels.contains(&"tree"));
}

#[test]
fn sorts_by_score() {
	let items = vec![make_item("xtracing"), make_item("tracing"), make_item("tr")];
	let filtered = filter_items(&items, "tr");
	// Exact match "tr" should be first, then prefix match "tracing"
	assert!(filtered[0].score >= filtered[1].score);
}

#[test]
fn extract_query_basic() {
	let rope = Rope::from_str("use tracing");
	let query = extract_query(&rope, 4, 11);
	assert_eq!(query, "tracing");
}

#[test]
fn extract_query_partial() {
	let rope = Rope::from_str("use tr");
	let query = extract_query(&rope, 4, 6);
	assert_eq!(query, "tr");
}

#[test]
fn extract_query_empty() {
	let rope = Rope::from_str("foo.");
	let query = extract_query(&rope, 4, 4);
	assert_eq!(query, "");
}

#[test]
fn match_indices_computed() {
	let items = vec![make_item("tracing"), make_item("tree")];
	let filtered = filter_items(&items, "tr");
	// Both items should have match indices
	assert!(filtered.iter().all(|f| f.match_indices.is_some()));
	// "tr" should match first two chars
	let tracing_indices = filtered
		.iter()
		.find(|f| items[f.index].label == "tracing")
		.and_then(|f| f.match_indices.as_ref());
	assert!(tracing_indices.is_some());
	let indices = tracing_indices.unwrap();
	assert!(indices.contains(&0)); // 't'
	assert!(indices.contains(&1)); // 'r'
}

#[test]
fn allows_single_typo_by_default() {
	let items = vec![make_item("registry_diag"), make_item("write")];
	let filtered = filter_items(&items, "rtgistry");
	assert_eq!(filtered.len(), 1);
	assert_eq!(items[filtered[0].index].label, "registry_diag");
}

#[test]
fn short_queries_do_not_allow_typos() {
	let items = vec![make_item("a"), make_item("ab")];
	let filtered = filter_items(&items, "ab");
	let labels: Vec<_> = filtered.iter().map(|f| items[f.index].label.as_str()).collect();
	assert!(labels.contains(&"ab"));
	assert!(!labels.contains(&"a"));
}

#[test]
fn lsp_symbol_with_single_typo_still_matches() {
	let items = vec![make_item("self::"), make_item("super::")];
	let filtered = filter_items(&items, "solf");
	assert_eq!(filtered.len(), 1);
	assert_eq!(items[filtered[0].index].label, "self::");
}
