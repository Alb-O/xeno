use evildoer_base::{Rope, Selection};
use evildoer_manifest::CompletionItem;

/// A history entry for undo/redo.
#[derive(Clone)]
pub struct HistoryEntry {
	pub doc: Rope,
	pub selection: Selection,
}

#[derive(Default)]
pub struct Registers {
	pub yank: String,
}

#[derive(Clone, Default)]
pub struct CompletionState {
	pub items: Vec<CompletionItem>,
	pub selected_idx: Option<usize>,
	pub active: bool,
	/// Start position in the input where replacement begins.
	/// When a completion is accepted, text from this position to cursor is replaced.
	pub replace_start: usize,
	/// Scroll offset for the completion menu viewport.
	/// This is the index of the first visible item when there are more items than can fit.
	pub scroll_offset: usize,
}

impl CompletionState {
	/// Maximum number of visible items in the completion menu.
	pub const MAX_VISIBLE: usize = 10;

	/// Ensures the selected item is visible within the viewport by adjusting scroll_offset.
	pub fn ensure_selected_visible(&mut self) {
		let Some(selected) = self.selected_idx else {
			return;
		};

		// If selection is above the viewport, scroll up
		if selected < self.scroll_offset {
			self.scroll_offset = selected;
		}

		// If selection is below the viewport, scroll down
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
