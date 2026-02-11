//! Completion infrastructure.
//!
//! Follows the rustyline pattern where `complete()` returns both the start
//! position in the input where replacement begins and the list of candidates.
//! This cleanly separates "where to replace" from "what to replace with".

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

use xeno_registry::commands::COMMANDS;

/// Prompt character for ex-style commands (`:write`, `:theme`, etc.).
pub const PROMPT_COMMAND: char = ':';

/// Type of completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
	/// Ex command completion.
	Command,
	/// File path completion.
	File,
	/// Open buffer completion.
	Buffer,
	/// Code snippet completion.
	Snippet,
	/// Theme name completion.
	Theme,
}

/// A single completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
	/// The text to display in the menu.
	pub label: String,
	/// The text to insert into the document (replaces from `start` to cursor).
	pub insert_text: String,
	/// Optional detail shown next to the label (e.g., command description).
	pub detail: Option<String>,
	/// Text used for filtering if different from label.
	pub filter_text: Option<String>,
	/// Kind of item.
	pub kind: CompletionKind,
	/// Indices of matched characters in the label for highlighting.
	pub match_indices: Option<Vec<usize>>,
	/// Optional right-aligned metadata shown in compact completion lists.
	pub right: Option<String>,
}

/// Result of a completion query.
#[derive(Debug, Clone, Default)]
pub struct CompletionResult {
	/// Start position in the input where replacement begins.
	/// All items in this result replace from this position to the cursor.
	pub start: usize,
	/// The completion candidates.
	pub items: Vec<CompletionItem>,
}

impl CompletionResult {
	/// Create a new completion result with the given start position and items.
	pub fn new(start: usize, items: Vec<CompletionItem>) -> Self {
		Self { start, items }
	}

	/// Create an empty result (no completions).
	pub fn empty() -> Self {
		Self::default()
	}

	/// Check if this result has any completions.
	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}
}

/// Context for generating completions.
#[derive(Debug, Clone)]
pub struct CompletionContext {
	/// Current input string being completed.
	pub input: String,
	/// Cursor position within the input string.
	pub cursor: usize,
	/// The prompt character (e.g., ':', '/', etc.).
	pub prompt: char,
}

/// Provides completion items for a specific context.
///
/// Implementors return a `CompletionResult` containing:
/// - `start`: The position in the input where replacement begins
/// - `items`: The list of completion candidates
///
/// When a completion is accepted, the text from `start` to the cursor
/// is replaced with the selected item's `insert_text`.
pub trait CompletionSource {
	/// Generate completions for the given context.
	///
	/// Returns the start position and list of candidates.
	/// Example: for input "theme gr" completing themes, returns `(6, [gruvbox, ...])`
	/// indicating replacement starts at position 6 (after "theme ").
	fn complete(&self, ctx: &CompletionContext) -> CompletionResult;
}

/// Tracks how the current completion selection was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionIntent {
	/// Selection set automatically.
	#[default]
	Auto,
	/// User explicitly navigated to this item.
	Manual,
}

/// State for managing the completion menu.
#[derive(Clone)]
pub struct CompletionState {
	/// Available completion items.
	pub items: Vec<CompletionItem>,
	/// Index of the currently selected item, if any.
	pub selected_idx: Option<usize>,
	/// Whether the completion menu is active and visible.
	pub active: bool,
	/// Start position in the input where replacement begins.
	pub replace_start: usize,
	/// Scroll offset for the completion menu viewport.
	pub scroll_offset: usize,
	/// How the current selection was made.
	pub selection_intent: SelectionIntent,
	/// Suppresses auto-popup until trigger char or manual invoke.
	pub suppressed: bool,
	/// Current filter query (text from replace_start to cursor).
	pub query: String,
	/// Whether to render the kind column in completion rows.
	pub show_kind: bool,
}

impl Default for CompletionState {
	fn default() -> Self {
		Self {
			items: Vec::new(),
			selected_idx: None,
			active: false,
			replace_start: 0,
			scroll_offset: 0,
			selection_intent: SelectionIntent::Auto,
			suppressed: false,
			query: String::new(),
			show_kind: true,
		}
	}
}

impl CompletionState {
	/// Maximum number of visible items in the completion menu.
	pub const MAX_VISIBLE: usize = 10;

	/// Ensures the selected item is visible within the viewport.
	pub fn ensure_selected_visible(&mut self) {
		let Some(selected) = self.selected_idx else {
			return;
		};
		if selected < self.scroll_offset {
			self.scroll_offset = selected;
		}
		let visible_end = self.scroll_offset + Self::MAX_VISIBLE;
		if selected >= visible_end {
			self.scroll_offset = selected.saturating_sub(Self::MAX_VISIBLE - 1);
		}
	}

	/// Returns the range of visible items (start..end indices).
	pub fn visible_range(&self) -> std::ops::Range<usize> {
		let end = (self.scroll_offset + Self::MAX_VISIBLE).min(self.items.len());
		self.scroll_offset..end
	}
}

/// Shared frizbee matcher baseline for editor completion paths.
pub(crate) fn frizbee_config() -> &'static frizbee::Config {
	static CONFIG: OnceLock<frizbee::Config> = OnceLock::new();
	CONFIG.get_or_init(|| frizbee::Config {
		prefilter: true,
		max_typos: Some(0),
		sort: true,
		scoring: frizbee::Scoring {
			delimiters: "_:./<>".to_string(),
			..Default::default()
		},
	})
}

/// Chooses typo tolerance based on query length to reduce noise on short queries.
fn max_typos_for_query(query: &str) -> u16 {
	match query.chars().count() {
		0..=3 => 0,
		4..=9 => 1,
		_ => 2,
	}
}

/// Builds a query-aware frizbee config for completion matching.
pub(crate) fn frizbee_config_for_query(query: &str) -> frizbee::Config {
	let mut config = frizbee_config().clone();
	config.max_typos = Some(max_typos_for_query(query));
	config
}

/// Matches a query against a haystack using frizbee.
///
/// Returns score/exact/match-indices when matched. Empty query always matches.
pub(crate) fn frizbee_match(query: &str, haystack: &str) -> Option<(u16, bool, Vec<usize>)> {
	if query.is_empty() {
		return Some((0, false, Vec::new()));
	}
	let config = frizbee_config_for_query(query);
	frizbee::match_indices(query, haystack, &config).map(|m| (m.score, m.exact, m.indices))
}

/// In-memory command usage store for command palette ranking.
#[derive(Clone, Default)]
pub struct CommandPaletteUsage {
	counts: HashMap<String, u32>,
	recent: VecDeque<String>,
}

impl CommandPaletteUsage {
	const MAX_RECENT: usize = 50;

	pub fn record(&mut self, name: &str) {
		*self.counts.entry(name.to_string()).or_insert(0) += 1;
		if let Some(idx) = self.recent.iter().position(|item| item == name) {
			self.recent.remove(idx);
		}
		self.recent.push_front(name.to_string());
		while self.recent.len() > Self::MAX_RECENT {
			self.recent.pop_back();
		}
	}

	pub fn snapshot(&self) -> CommandUsageSnapshot {
		CommandUsageSnapshot {
			counts: self.counts.clone(),
			recent: self.recent.iter().cloned().collect(),
		}
	}
}

/// Read-only command usage view for ranking.
#[derive(Clone, Default)]
pub struct CommandUsageSnapshot {
	pub counts: HashMap<String, u32>,
	pub recent: Vec<String>,
}

impl CommandUsageSnapshot {
	pub fn count(&self, name: &str) -> u32 {
		self.counts.get(name).copied().unwrap_or(0)
	}

	pub fn recent_rank(&self, name: &str) -> Option<usize> {
		self.recent.iter().position(|item| item == name)
	}
}

/// Completion source for editor commands.
pub struct CommandSource;

impl CompletionSource for CommandSource {
	fn complete(&self, ctx: &CompletionContext) -> CompletionResult {
		if ctx.prompt != PROMPT_COMMAND {
			return CompletionResult::empty();
		}

		let input = &ctx.input;

		// Only complete command names if we haven't typed a space yet
		// (once there's a space, we're completing arguments, not the command)
		if input.contains(' ') {
			return CompletionResult::empty();
		}

		let mut scored: Vec<(i32, CompletionItem)> = COMMANDS
			.snapshot_guard()
			.iter_refs()
			.filter_map(|cmd| {
				let name = cmd.name_str();
				let mut best = i32::MIN;
				let mut match_indices = None;

				if let Some((score, _, indices)) = frizbee_match(input, name) {
					best = score as i32 + 200;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}
				for alias in cmd.keys_resolved() {
					if let Some((score, _, _)) = frizbee_match(input, alias) {
						best = best.max(score as i32 + 80);
					}
				}
				if input.is_empty() {
					best = 0;
				}
				if !input.is_empty() && best == i32::MIN {
					return None;
				}

				Some((
					best,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some(cmd.description_str().to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices,
						right: None,
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));
		let items = scored.into_iter().map(|(_, item)| item).collect();

		// Command completions replace from position 0 (entire input)
		CompletionResult::new(0, items)
	}
}

#[cfg(test)]
mod tests {
	use super::{frizbee_config_for_query, frizbee_match, max_typos_for_query};

	#[test]
	fn matching_allows_single_typo_by_default() {
		assert!(frizbee_match("rtgistry", "registry_diag").is_some());
	}

	#[test]
	fn short_queries_do_not_allow_typos() {
		assert!(frizbee_match("ab", "a").is_none());
	}

	#[test]
	fn typo_budget_scales_with_query_length() {
		assert_eq!(max_typos_for_query("abc"), 0);
		assert_eq!(max_typos_for_query("abcd"), 1);
		assert_eq!(max_typos_for_query("abcdefghij"), 2);

		assert_eq!(frizbee_config_for_query("ab").max_typos, Some(0));
		assert_eq!(frizbee_config_for_query("rtgistry").max_typos, Some(1));
		assert_eq!(frizbee_config_for_query("very_long_query").max_typos, Some(2));
	}
}
