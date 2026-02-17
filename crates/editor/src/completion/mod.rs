//! Completion infrastructure.
//!
//! Follows the rustyline pattern where `complete()` returns both the start
//! position in the input where replacement begins and the list of candidates.
//! This cleanly separates "where to replace" from "what to replace with".

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
pub struct CompletionFileMeta {
	path: PathBuf,
	kind: xeno_file_display::FileKind,
}

impl CompletionFileMeta {
	pub fn new(path: impl Into<PathBuf>, kind: xeno_file_display::FileKind) -> Self {
		Self { path: path.into(), kind }
	}

	pub fn path(&self) -> &Path {
		&self.path
	}

	pub fn kind(&self) -> xeno_file_display::FileKind {
		self.kind
	}
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
	/// Structured file metadata for file completion items.
	pub file: Option<CompletionFileMeta>,
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
	/// Mapping from displayed LSP completion row index to raw LSP item index.
	///
	/// This is populated only for active LSP completion menus and is empty for
	/// non-LSP completion sources.
	#[cfg(feature = "lsp")]
	pub lsp_display_to_raw: Vec<usize>,
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
			#[cfg(feature = "lsp")]
			lsp_display_to_raw: Vec::new(),
		}
	}
}

impl CompletionState {
	/// Maximum number of visible items in the completion menu.
	pub const MAX_VISIBLE: usize = 10;

	/// Returns `max_visible` clamped to at least 1 row.
	fn normalize_visible_limit(max_visible: usize) -> usize {
		max_visible.max(1)
	}

	/// Ensures the selected item is visible within a bounded viewport.
	pub fn ensure_selected_visible_with_limit(&mut self, max_visible: usize) {
		let Some(selected) = self.selected_idx else {
			return;
		};
		let max_visible = Self::normalize_visible_limit(max_visible);
		if selected < self.scroll_offset {
			self.scroll_offset = selected;
		}
		let visible_end = self.scroll_offset + max_visible;
		if selected >= visible_end {
			self.scroll_offset = selected.saturating_sub(max_visible - 1);
		}
	}

	/// Returns the range of visible items for a bounded viewport.
	pub fn visible_range_with_limit(&self, max_visible: usize) -> std::ops::Range<usize> {
		let max_visible = Self::normalize_visible_limit(max_visible);
		let end = (self.scroll_offset + max_visible).min(self.items.len());
		self.scroll_offset..end
	}

	/// Ensures the selected item is visible within the viewport.
	pub fn ensure_selected_visible(&mut self) {
		self.ensure_selected_visible_with_limit(Self::MAX_VISIBLE);
	}
}

/// Data-only icon + label payload for file completion rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePresentationRender {
	pub(crate) icon: String,
	pub(crate) label: String,
}

impl FilePresentationRender {
	pub fn new(icon: String, label: String) -> Self {
		Self { icon, label }
	}

	pub fn icon(&self) -> &str {
		&self.icon
	}

	pub fn label(&self) -> &str {
		&self.label
	}
}

/// Data-only completion menu row used by frontend renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRenderItem {
	pub(crate) label: String,
	pub(crate) kind: CompletionKind,
	pub(crate) right: Option<String>,
	pub(crate) match_indices: Option<Vec<usize>>,
	pub(crate) selected: bool,
	pub(crate) command_alias_match: bool,
	pub(crate) file_presentation: Option<FilePresentationRender>,
}

impl CompletionRenderItem {
	pub fn from_parts(
		label: String,
		kind: CompletionKind,
		right: Option<String>,
		match_indices: Option<Vec<usize>>,
		selected: bool,
		command_alias_match: bool,
		file_presentation: Option<FilePresentationRender>,
	) -> Self {
		Self {
			label,
			kind,
			right,
			match_indices,
			selected,
			command_alias_match,
			file_presentation,
		}
	}

	pub fn label(&self) -> &str {
		&self.label
	}
	pub fn kind(&self) -> CompletionKind {
		self.kind
	}
	pub fn right(&self) -> Option<&str> {
		self.right.as_deref()
	}
	pub fn match_indices(&self) -> Option<&[usize]> {
		self.match_indices.as_deref()
	}
	pub fn selected(&self) -> bool {
		self.selected
	}
	pub fn command_alias_match(&self) -> bool {
		self.command_alias_match
	}
	pub fn file_presentation(&self) -> Option<&FilePresentationRender> {
		self.file_presentation.as_ref()
	}
}

/// Data-only completion menu render plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRenderPlan {
	pub(crate) items: Vec<CompletionRenderItem>,
	pub(crate) max_label_width: usize,
	pub(crate) target_row_width: usize,
	pub(crate) show_kind: bool,
	pub(crate) show_right: bool,
}

impl CompletionRenderPlan {
	pub fn new(items: Vec<CompletionRenderItem>, max_label_width: usize, target_row_width: usize, show_kind: bool, show_right: bool) -> Self {
		Self {
			items,
			max_label_width,
			target_row_width,
			show_kind,
			show_right,
		}
	}

	pub fn items(&self) -> &[CompletionRenderItem] {
		&self.items
	}
	pub fn max_label_width(&self) -> usize {
		self.max_label_width
	}
	pub fn target_row_width(&self) -> usize {
		self.target_row_width
	}
	pub fn show_kind(&self) -> bool {
		self.show_kind
	}
	pub fn show_right(&self) -> bool {
		self.show_right
	}
}

/// Shared xeno-matcher baseline for editor completion paths.
pub(crate) fn frizbee_config() -> &'static xeno_matcher::Config {
	static CONFIG: OnceLock<xeno_matcher::Config> = OnceLock::new();
	CONFIG.get_or_init(|| xeno_matcher::Config {
		prefilter: true,
		max_typos: Some(0),
		sort: true,
		scoring: xeno_matcher::Scoring {
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

/// Builds a query-aware xeno-matcher config for completion matching.
pub(crate) fn frizbee_config_for_query(query: &str) -> xeno_matcher::Config {
	let mut config = frizbee_config().clone();
	config.max_typos = Some(max_typos_for_query(query));
	config
}

/// Matches a query against a haystack using xeno-matcher.
///
/// Returns score/exact/match-indices when matched. Empty query always matches.
pub(crate) fn frizbee_match(query: &str, haystack: &str) -> Option<(u16, bool, Vec<usize>)> {
	if query.is_empty() {
		return Some((0, false, Vec::new()));
	}
	let config = frizbee_config_for_query(query);
	xeno_matcher::match_indices(query, haystack, &config).map(|m| (m.score, m.exact, m.indices))
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

#[cfg(test)]
mod tests;
