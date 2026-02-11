//! LSP UI event handling (completions, signature help).

use xeno_lsp::lsp_types::{CompletionItem as LspCompletionItem, CompletionList, CompletionResponse};
use xeno_primitives::range::CharIdx;

use super::completion_filter::{extract_query, filter_items};
use super::types::{LspMenuKind, LspMenuState};
use crate::buffer::ViewId;
use crate::completion::{CompletionState, SelectionIntent};
use crate::impls::Editor;
use crate::info_popup::PopupAnchor;
use crate::{CompletionItem, CompletionKind};

pub enum LspUiEvent {
	CompletionResult {
		generation: u64,
		buffer_id: ViewId,
		replace_start: usize,
		response: Option<CompletionResponse>,
	},
	SignatureHelp {
		generation: u64,
		buffer_id: ViewId,
		cursor: CharIdx,
		doc_version: u64,
		contents: String,
		anchor: PopupAnchor,
	},
}

impl Editor {
	pub(crate) fn drain_lsp_ui_events(&mut self) {
		while let Some(event) = self.state.lsp.try_recv_ui_event() {
			self.handle_lsp_ui_event(event);
		}
	}

	/// Processes an LSP UI event (completion results, signature help).
	///
	/// For completion results, validates the response against the current editor state:
	/// the generation must match the active request, and the cursor must still be at or
	/// after `replace_start` (allowing continued typing without dismissing the menu).
	/// Stale results from cancelled requests are silently discarded.
	fn handle_lsp_ui_event(&mut self, event: LspUiEvent) {
		if self.state.overlay_system.interaction.is_open() {
			return;
		}

		match event {
			LspUiEvent::CompletionResult {
				generation,
				buffer_id,
				replace_start,
				response,
			} => {
				if generation != self.state.lsp.completion_generation() {
					return;
				}
				let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
					return;
				};
				if buffer.cursor < replace_start {
					self.clear_lsp_menu();
					return;
				}

				let items = response.map(completion_items_from_response).unwrap_or_default();
				if items.is_empty() {
					self.clear_lsp_menu();
					return;
				}

				let query = buffer.with_doc(|doc| extract_query(doc.content(), replace_start, buffer.cursor));
				let filtered = filter_items(&items, &query);

				if filtered.is_empty() {
					self.clear_lsp_menu();
					return;
				}

				let display_items: Vec<CompletionItem> = filtered
					.iter()
					.map(|f| map_completion_item_with_indices(&items[f.index], f.match_indices.clone()))
					.collect();

				let completions = self.overlays_mut().get_or_default::<CompletionState>();
				completions.items = display_items;
				completions.selected_idx = None;
				completions.selection_intent = SelectionIntent::Auto;
				completions.active = true;
				completions.replace_start = replace_start;
				completions.scroll_offset = 0;
				completions.query = query;

				let menu_state = self.overlays_mut().get_or_default::<LspMenuState>();
				menu_state.set(LspMenuKind::Completion { buffer_id, items });

				self.state.frame.needs_redraw = true;
			}
			LspUiEvent::SignatureHelp {
				generation,
				buffer_id,
				cursor,
				doc_version,
				contents,
				anchor,
			} => {
				if generation != self.state.lsp.signature_help_generation() {
					return;
				}
				let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
					return;
				};
				if buffer.version() != doc_version || buffer.cursor != cursor {
					return;
				}
				if contents.is_empty() {
					return;
				}
				self.open_info_popup(contents, Some("markdown"), anchor);
			}
		}
	}

	pub(crate) fn clear_lsp_menu(&mut self) {
		if let Some(completions) = self.overlays().get::<CompletionState>()
			&& completions.active
		{
			let completions = self.overlays_mut().get_or_default::<CompletionState>();
			completions.items.clear();
			completions.selected_idx = None;
			completions.active = false;
			completions.scroll_offset = 0;
			completions.replace_start = 0;
			completions.query.clear();
		}

		if let Some(menu_state) = self.overlays().get::<LspMenuState>()
			&& menu_state.is_active()
		{
			let menu_state = self.overlays_mut().get_or_default::<LspMenuState>();
			menu_state.clear();
		}

		self.state.frame.needs_redraw = true;
	}
}

fn completion_items_from_response(response: CompletionResponse) -> Vec<LspCompletionItem> {
	match response {
		CompletionResponse::Array(items) => items,
		CompletionResponse::List(CompletionList { items, .. }) => items,
	}
}

/// Converts an LSP [`CompletionItem`](LspCompletionItem) to the UI [`CompletionItem`] type.
///
/// Extracts label, insert text, detail, and kind from the LSP item. The `match_indices`
/// are passed through for highlight rendering in the completion menu.
pub(crate) fn map_completion_item_with_indices(item: &LspCompletionItem, match_indices: Option<Vec<usize>>) -> CompletionItem {
	let insert_text = item.insert_text.clone().unwrap_or_else(|| item.label.clone());
	let kind = match item.insert_text_format {
		Some(xeno_lsp::lsp_types::InsertTextFormat::SNIPPET) => CompletionKind::Snippet,
		_ => CompletionKind::Command,
	};

	CompletionItem {
		label: item.label.clone(),
		insert_text,
		detail: item.detail.clone(),
		filter_text: item.filter_text.clone(),
		kind,
		match_indices,
	}
}
