//! LSP UI event handling (completions, signature help).

use xeno_base::range::CharIdx;
use xeno_core::{CompletionItem, CompletionKind};
use xeno_lsp::lsp_types::{
	CompletionItem as LspCompletionItem, CompletionList, CompletionResponse,
};

use crate::buffer::BufferId;
use crate::editor::Editor;
use crate::editor::types::{CompletionState, LspMenuKind, LspMenuState};
use crate::info_popup::PopupAnchor;

pub enum LspUiEvent {
	CompletionResult {
		generation: u64,
		buffer_id: BufferId,
		cursor: CharIdx,
		doc_version: u64,
		replace_start: usize,
		response: Option<CompletionResponse>,
	},
	SignatureHelp {
		generation: u64,
		buffer_id: BufferId,
		cursor: CharIdx,
		doc_version: u64,
		contents: String,
		anchor: PopupAnchor,
	},
}

impl Editor {
	pub(crate) fn drain_lsp_ui_events(&mut self) {
		while let Ok(event) = self.lsp_ui_rx.try_recv() {
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
		match event {
			LspUiEvent::CompletionResult {
				generation,
				buffer_id,
				cursor: _,
				doc_version: _,
				replace_start,
				response,
			} => {
				if generation != self.completion_controller.generation() {
					return;
				}
				let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
					return;
				};
				if buffer.cursor < replace_start {
					self.clear_lsp_menu();
					return;
				}

				let items = response
					.map(completion_items_from_response)
					.unwrap_or_default();
				if items.is_empty() {
					self.clear_lsp_menu();
					return;
				}

				let display_items: Vec<CompletionItem> =
					items.iter().map(map_completion_item).collect();

				let completions = self.overlays.get_or_default::<CompletionState>();
				completions.items = display_items;
				completions.selected_idx = Some(0);
				completions.active = true;
				completions.replace_start = replace_start;
				completions.scroll_offset = 0;

				let menu_state = self.overlays.get_or_default::<LspMenuState>();
				menu_state.set(LspMenuKind::Completion { buffer_id, items });

				self.frame.needs_redraw = true;
			}
			LspUiEvent::SignatureHelp {
				generation,
				buffer_id,
				cursor,
				doc_version,
				contents,
				anchor,
			} => {
				if generation != self.signature_help_generation {
					return;
				}
				let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
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
		if let Some(completions) = self.overlays.get::<CompletionState>()
			&& completions.active
		{
			let completions = self.overlays.get_or_default::<CompletionState>();
			completions.items.clear();
			completions.selected_idx = None;
			completions.active = false;
			completions.scroll_offset = 0;
			completions.replace_start = 0;
		}

		if let Some(menu_state) = self.overlays.get::<LspMenuState>()
			&& menu_state.is_active()
		{
			let menu_state = self.overlays.get_or_default::<LspMenuState>();
			menu_state.clear();
		}

		self.frame.needs_redraw = true;
	}
}

fn completion_items_from_response(response: CompletionResponse) -> Vec<LspCompletionItem> {
	match response {
		CompletionResponse::Array(items) => items,
		CompletionResponse::List(CompletionList { items, .. }) => items,
	}
}

fn map_completion_item(item: &LspCompletionItem) -> CompletionItem {
	let insert_text = item
		.insert_text
		.clone()
		.unwrap_or_else(|| item.label.clone());
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
	}
}
