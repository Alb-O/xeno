//! LSP-related type definitions.
//!
//! - [`CompletionState`] - Completion menu state and viewport
//! - [`LspMenuState`] - Active LSP menu (completions or code actions)

use xeno_lsp::lsp_types::{CodeActionOrCommand, CompletionItem as LspCompletionItem};

use crate::buffer::BufferId;
use crate::CompletionItem;

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

/// The kind of LSP-driven menu currently active.
#[derive(Clone)]
pub enum LspMenuKind {
	Completion {
		buffer_id: BufferId,
		items: Vec<LspCompletionItem>,
	},
	CodeAction {
		buffer_id: BufferId,
		actions: Vec<CodeActionOrCommand>,
	},
}

/// State for tracking the active LSP menu.
#[derive(Clone, Default)]
pub struct LspMenuState {
	kind: Option<LspMenuKind>,
}

impl LspMenuState {
	pub fn set(&mut self, kind: LspMenuKind) {
		self.kind = Some(kind);
	}

	pub fn clear(&mut self) {
		self.kind = None;
	}

	pub fn active(&self) -> Option<&LspMenuKind> {
		self.kind.as_ref()
	}

	pub fn is_active(&self) -> bool {
		self.kind.is_some()
	}
}
