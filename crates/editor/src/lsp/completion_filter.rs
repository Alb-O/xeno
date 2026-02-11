//! Fuzzy filtering for LSP completion items using frizbee.

use ropey::Rope;
use xeno_lsp::lsp_types::CompletionItem as LspCompletionItem;

/// Result of filtering a completion item.
#[derive(Debug, Clone)]
pub struct FilteredItem {
	/// Index into the original raw items list.
	pub index: usize,
	/// Match score (higher = better match).
	#[allow(dead_code, reason = "useful for debugging and future scoring display")]
	pub score: u16,
	/// Whether this was an exact match.
	#[allow(dead_code, reason = "useful for future exact-match styling")]
	pub exact: bool,
	/// Indices of matched characters in the label.
	pub match_indices: Option<Vec<usize>>,
}

/// Filters completion items using fuzzy matching.
///
/// Returns items sorted by match score (descending). Empty query returns
/// all items in original order with score 0. Includes matched character
/// indices for highlighting.
pub fn filter_items(raw_items: &[LspCompletionItem], query: &str) -> Vec<FilteredItem> {
	if query.is_empty() {
		return raw_items
			.iter()
			.enumerate()
			.map(|(index, _)| FilteredItem {
				index,
				score: 0,
				exact: false,
				match_indices: None,
			})
			.collect();
	}

	let filter_texts: Vec<&str> = raw_items
		.iter()
		.map(|item| item.filter_text.as_deref().unwrap_or(item.label.as_str()))
		.collect();

	let config = crate::completion::frizbee_config();
	let matches = frizbee::match_list(query, &filter_texts, config);

	matches
		.into_iter()
		.map(|m| {
			let idx = m.index as usize;
			let match_indices = frizbee::match_indices(query, &raw_items[idx].label, config).map(|mi| mi.indices);
			FilteredItem {
				index: idx,
				score: m.score,
				exact: m.exact,
				match_indices,
			}
		})
		.collect()
}

/// Extracts the completion query from the buffer.
///
/// Query is the text between `replace_start` and `cursor`.
pub fn extract_query(rope: &Rope, replace_start: usize, cursor: usize) -> String {
	if cursor <= replace_start {
		return String::new();
	}
	let start = replace_start.min(rope.len_chars());
	let end = cursor.min(rope.len_chars());
	rope.slice(start..end).to_string()
}

#[cfg(test)]
mod tests {
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
}
