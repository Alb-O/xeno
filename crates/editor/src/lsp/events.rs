//! LSP UI event handling (completions, signature help).

use std::sync::Arc;

use xeno_lsp::lsp_types::{CompletionList, CompletionResponse};
use xeno_primitives::CharIdx;

use super::completion_filter::{extract_query, filter_items};
use super::types::{LspMenuKind, LspMenuState};
use crate::Editor;
use crate::buffer::ViewId;
use crate::completion::{CompletionItem, CompletionState, SelectionIntent};
use crate::info_popup::PopupAnchor;
use crate::render::InlayHintRangeMap;
use crate::render_api::CompletionKind;

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
	InlayHintResult {
		generation: u64,
		buffer_id: ViewId,
		doc_rev: u64,
		line_lo: usize,
		line_hi: usize,
		hints: Arc<InlayHintRangeMap>,
	},
	PullDiagnosticResult {
		buffer_id: ViewId,
		doc_rev: u64,
		outcome: PullDiagnosticOutcome,
	},
	SemanticTokensResult {
		generation: u64,
		epoch: u64,
		buffer_id: ViewId,
		doc_rev: u64,
		line_lo: usize,
		line_hi: usize,
		tokens: Arc<super::semantic_tokens::SemanticTokenSpans>,
	},
	DocumentHighlightResult {
		generation: u64,
		buffer_id: ViewId,
		doc_rev: u64,
		cursor: usize,
		highlights: super::document_highlight::DocumentHighlightSpans,
	},
}

/// Outcome of a pull diagnostics request.
#[derive(Debug)]
pub(crate) enum PullDiagnosticOutcome {
	/// Server returned a full diagnostic report.
	Full {
		result_id: Option<String>,
		diagnostics: Vec<xeno_lsp::lsp_types::Diagnostic>,
	},
	/// Server indicates diagnostics are unchanged since `result_id`.
	Unchanged { result_id: String },
	/// Request failed — allow retry on next tick.
	Failed,
}

impl Editor {
	pub(crate) fn drain_lsp_ui_events(&mut self) {
		while let Some(event) = self.state.integration.lsp.try_recv_ui_event() {
			self.handle_lsp_ui_event(event);
		}
	}

	/// Drains pending workspace/applyEdit requests from the LSP server request handler.
	///
	/// Each request is enqueued in the runtime work queue with its reply channel
	/// stored in a side map. The reply is sent after the edit is actually applied
	/// (or fails) during the runtime drain phase, providing honest semantics to
	/// the LSP server.
	pub(crate) fn drain_lsp_apply_edits(&mut self) {
		while let Some(request) = self.state.integration.lsp.try_recv_apply_edit() {
			self.enqueue_runtime_workspace_edit_work(request.edit, Some((request.reply, request.deadline)));
		}
	}

	/// Processes an LSP UI event (completion results, signature help).
	///
	/// For completion results, validates the response against the current editor state:
	/// the generation must match the active request, and the cursor must still be at or
	/// after `replace_start` (allowing continued typing without dismissing the menu).
	/// Stale results from cancelled requests are silently discarded.
	fn handle_lsp_ui_event(&mut self, event: LspUiEvent) {
		// Inlay hint + pull diagnostic results are always processed (even with overlay open).
		match event {
			LspUiEvent::InlayHintResult {
				generation,
				buffer_id,
				doc_rev,
				line_lo,
				line_hi,
				hints,
			} => {
				let current_gen = self.state.ui.inlay_hint_cache.generation(buffer_id);
				if generation >= current_gen {
					self.state.ui.inlay_hint_cache.insert(buffer_id, doc_rev, line_lo, line_hi, generation, hints);
					self.state.core.frame.needs_redraw = true;
				}
				return;
			}
			LspUiEvent::PullDiagnosticResult { buffer_id, doc_rev, outcome } => {
				self.handle_pull_diagnostic_result(buffer_id, doc_rev, &outcome);
				return;
			}
			LspUiEvent::SemanticTokensResult {
				generation,
				epoch,
				buffer_id,
				doc_rev,
				line_lo,
				line_hi,
				tokens,
			} => {
				let current_gen = self.state.ui.semantic_token_cache.generation(buffer_id);
				if generation >= current_gen {
					self.state
						.ui
						.semantic_token_cache
						.insert(buffer_id, doc_rev, line_lo, line_hi, generation, epoch, tokens);
					self.state.core.frame.needs_redraw = true;
				}
				return;
			}
			LspUiEvent::DocumentHighlightResult {
				generation,
				buffer_id,
				doc_rev,
				cursor,
				highlights,
			} => {
				let current_gen = self.state.ui.document_highlight_cache.generation(buffer_id);
				if generation >= current_gen {
					self.state
						.ui
						.document_highlight_cache
						.insert(buffer_id, doc_rev, cursor, generation, highlights);
					self.state.core.frame.needs_redraw = true;
				}
				return;
			}
			other => {
				// Fall through for completion/signature help, which need overlay check.
				if self.state.ui.overlay_system.interaction().is_open() {
					return;
				}
				self.handle_lsp_ui_event_inner(other);
			}
		}
	}

	fn handle_lsp_ui_event_inner(&mut self, event: LspUiEvent) {
		match event {
			LspUiEvent::CompletionResult {
				generation,
				buffer_id,
				replace_start,
				response,
			} => {
				if generation != self.state.integration.lsp.completion_generation() {
					return;
				}
				let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
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
				completions.lsp_display_to_raw = filtered.iter().map(|f| f.index).collect();
				completions.selected_idx = None;
				completions.selection_intent = SelectionIntent::Auto;
				completions.active = true;
				completions.replace_start = replace_start;
				completions.scroll_offset = 0;
				completions.query = query;

				let menu_state = self.overlays_mut().get_or_default::<LspMenuState>();
				menu_state.set(LspMenuKind::Completion { buffer_id, items });

				self.state.core.frame.needs_redraw = true;
			}
			LspUiEvent::SignatureHelp {
				generation,
				buffer_id,
				cursor,
				doc_version,
				contents,
				anchor,
			} => {
				if generation != self.state.integration.lsp.signature_help_generation() {
					return;
				}
				let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
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
			// Handled before the overlay-open check above.
			LspUiEvent::InlayHintResult { .. }
			| LspUiEvent::PullDiagnosticResult { .. }
			| LspUiEvent::SemanticTokensResult { .. }
			| LspUiEvent::DocumentHighlightResult { .. } => {
				unreachable!()
			}
		}
	}

	/// Processes a pull diagnostic result, updating state and requesting redraw.
	fn handle_pull_diagnostic_result(&mut self, buffer_id: ViewId, doc_rev: u64, outcome: &PullDiagnosticOutcome) {
		match outcome {
			PullDiagnosticOutcome::Full { result_id, .. } => {
				// Diagnostics already applied to DocumentStateManager in the spawned task.
				self.state.ui.pull_diag_state.record_full(buffer_id, doc_rev, result_id.clone());
				self.state.core.frame.needs_redraw = true;
			}
			PullDiagnosticOutcome::Unchanged { result_id } => {
				// No diagnostics to apply — keep existing.
				self.state.ui.pull_diag_state.record_unchanged(buffer_id, doc_rev, result_id.clone());
			}
			PullDiagnosticOutcome::Failed => {
				self.state.ui.pull_diag_state.record_failed(buffer_id);
			}
		}
	}

	pub(crate) fn clear_lsp_menu(&mut self) {
		if let Some(completions) = self.overlays().get::<CompletionState>()
			&& completions.active
		{
			let completions = self.overlays_mut().get_or_default::<CompletionState>();
			completions.items.clear();
			completions.lsp_display_to_raw.clear();
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

		self.state.core.frame.needs_redraw = true;
	}
}

fn completion_items_from_response(response: CompletionResponse) -> Vec<xeno_lsp::lsp_types::CompletionItem> {
	match response {
		CompletionResponse::Array(items) => items,
		CompletionResponse::List(CompletionList { items, .. }) => items,
	}
}

/// Converts an LSP [`xeno_lsp::lsp_types::CompletionItem`] to the UI [`CompletionItem`] type.
///
/// Extracts label, insert text, detail, and kind from the LSP item. The `match_indices`
/// are passed through for highlight rendering in the completion menu.
pub(crate) fn map_completion_item_with_indices(item: &xeno_lsp::lsp_types::CompletionItem, match_indices: Option<Vec<usize>>) -> CompletionItem {
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
		right: None,
		file: None,
	}
}
