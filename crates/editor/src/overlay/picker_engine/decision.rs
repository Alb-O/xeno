//! Selection helpers shared by picker overlays.

use crate::completion::{CompletionItem, CompletionState};

/// Returns active selected completion item, falling back to first visible item.
pub fn selected_completion_item(state: Option<&CompletionState>) -> Option<CompletionItem> {
	state
		.and_then(|state| {
			if !state.active {
				return None;
			}
			state.selected_idx.and_then(|idx| state.items.get(idx)).or_else(|| state.items.first())
		})
		.cloned()
}

/// Returns true when input already equals selected insertion text.
pub fn is_exact_selection_match(current_input: &str, selected: &CompletionItem) -> bool {
	current_input == selected.insert_text
}
