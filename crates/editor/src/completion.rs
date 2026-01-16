//! Completion infrastructure.
//!
//! Follows the rustyline pattern where `complete()` returns both the start
//! position in the input where replacement begins and the list of candidates.
//! This cleanly separates "where to replace" from "what to replace with".

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
#[derive(Clone, Default)]
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

		let items: Vec<_> = COMMANDS
			.iter()
			.filter(|cmd| {
				cmd.name.starts_with(input) || cmd.aliases.iter().any(|a| a.starts_with(input))
			})
			.map(|cmd| CompletionItem {
				label: cmd.name.to_string(),
				insert_text: cmd.name.to_string(),
				detail: Some(cmd.description.to_string()),
				filter_text: None,
				kind: CompletionKind::Command,
				match_indices: None,
			})
			.collect();

		// Command completions replace from position 0 (entire input)
		CompletionResult::new(0, items)
	}
}
