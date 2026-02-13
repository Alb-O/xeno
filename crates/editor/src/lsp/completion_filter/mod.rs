//! Fuzzy filtering for LSP completion items using xeno-matcher.

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

	let config = crate::completion::frizbee_config_for_query(query);
	let matches = xeno_matcher::match_list(query, &filter_texts, &config);

	matches
		.into_iter()
		.map(|m| {
			let idx = m.index as usize;
			let match_indices = xeno_matcher::match_indices(query, &raw_items[idx].label, &config).map(|mi| mi.indices);
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
mod tests;
