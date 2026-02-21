//! LSP completion triggering and application.
//!
//! Completions flow through several stages: the user types or manually triggers (Ctrl+Space),
//! this module sends a request to the language server via [`CompletionController`], results
//! arrive asynchronously and are filtered against the current query, then the user selects
//! an item which gets applied to the buffer.
//!
//! Snippet completions receive special handling—the snippet syntax (`$1`, `${2:default}`)
//! is parsed and converted to multi-cursor selections after insertion.
//!
//! [`CompletionController`]: super::completion_controller::CompletionController

use xeno_lsp::lsp_types::{CompletionItem, CompletionTextEdit, CompletionTriggerKind, InsertTextFormat, TextEdit};
use xeno_lsp::{CompletionRequest, CompletionTrigger, OffsetEncoding};
use xeno_primitives::{Bias, CharIdx, Selection};
use xeno_registry::notifications::keys;

use super::completion_filter::{extract_query, filter_items};
use super::events::map_completion_item_with_indices;
use super::types::{LspMenuKind, LspMenuState};
use super::workspace_edit::{ApplyError, BufferEditPlan, PlannedTextEdit, convert_text_edit};
use crate::Editor;
use crate::buffer::ViewId;
use crate::completion::{CompletionState, SelectionIntent};
use crate::snippet::vars::EditorSnippetResolver;
use crate::snippet::{parse_snippet_template, render_with_resolver};

impl Editor {
	pub(crate) fn is_completion_trigger_key(&self, key: &xeno_primitives::Key) -> bool {
		use xeno_primitives::KeyCode;
		matches!(key.code, KeyCode::Char(' ') | KeyCode::Space) && key.modifiers.ctrl && !key.modifiers.alt && !key.modifiers.shift
	}

	pub(crate) fn trigger_lsp_completion(&mut self, trigger: CompletionTrigger, trigger_char: Option<char>) {
		let is_trigger_char = trigger_char.is_some_and(is_completion_trigger_char);
		let is_manual = matches!(trigger, CompletionTrigger::Manual);

		if is_trigger_char || is_manual {
			self.overlays_mut().get_or_default::<CompletionState>().suppressed = false;
		} else if self.overlays().get::<CompletionState>().is_some_and(|s| s.suppressed) {
			return;
		}

		let buffer = self.buffer();
		if buffer.mode() != xeno_primitives::Mode::Insert {
			return;
		}
		if buffer.path().is_none() || buffer.file_type().is_none() {
			return;
		}

		let Some((client, uri, position)) = self.state.integration.lsp.prepare_position_request(buffer).ok().flatten() else {
			return;
		};

		if !client.supports_completion() {
			return;
		}

		let selection = buffer.selection.primary();
		let replace_start: usize = if selection.is_point() {
			completion_replace_start(buffer)
		} else {
			selection.from()
		};
		let request = CompletionRequest {
			id: self.focused_view(),
			replace_start,
			client,
			uri,
			position,
			debounce: trigger.debounce(),
			trigger_kind: completion_trigger_kind(&trigger, trigger_char),
			trigger_character: trigger_char.map(|c| c.to_string()),
		};

		self.state.integration.lsp.trigger_completion(request);
	}

	/// Refilters the active completion menu with the current query.
	///
	/// Called when the user types or deletes while a completion menu is visible,
	/// to update filtering without waiting for a new LSP response.
	pub(crate) fn refilter_completion(&mut self) {
		let menu_kind = self.overlays().get::<LspMenuState>().and_then(|s: &LspMenuState| s.active());
		let Some(LspMenuKind::Completion { buffer_id, items }) = menu_kind else {
			return;
		};
		let buffer_id = *buffer_id;
		let items = items.clone();

		let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
			return;
		};

		let replace_start = self.overlays().get::<CompletionState>().map(|s| s.replace_start).unwrap_or(0);

		if buffer.cursor < replace_start {
			self.clear_lsp_menu();
			return;
		}

		let query = buffer.with_doc(|doc| extract_query(doc.content(), replace_start, buffer.cursor));
		let filtered = filter_items(&items, &query);

		if filtered.is_empty() {
			self.clear_lsp_menu();
			return;
		}

		let display_items: Vec<crate::completion::CompletionItem> = filtered
			.iter()
			.map(|f| map_completion_item_with_indices(&items[f.index], f.match_indices.clone()))
			.collect();

		let completions = self.overlays_mut().get_or_default::<CompletionState>();
		completions.items = display_items;
		completions.lsp_display_to_raw = filtered.iter().map(|f| f.index).collect();
		completions.selected_idx = None;
		completions.selection_intent = SelectionIntent::Auto;
		completions.scroll_offset = 0;
		completions.query = query;

		self.state.core.frame.needs_redraw = true;
	}

	pub(crate) async fn apply_completion_item(&mut self, buffer_id: ViewId, item: CompletionItem) {
		let resolver = EditorSnippetResolver::new(self, buffer_id);
		let (encoding, selection, cursor, rope, readonly) = {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
				return;
			};
			(
				self.state.integration.lsp.offset_encoding_for_buffer(buffer),
				buffer.selection.primary(),
				buffer.cursor,
				buffer.with_doc(|doc| doc.content().clone()),
				buffer.is_readonly(),
			)
		};
		if readonly {
			self.notify(keys::BUFFER_READONLY);
			return;
		}
		let command = item.command.clone();

		let replace_start: CharIdx = if selection.is_point() {
			self.overlays()
				.get::<CompletionState>()
				.map(|state| state.replace_start)
				.unwrap_or_else(|| completion_replace_start_at(&rope, cursor))
		} else {
			selection.from()
		};
		let replace_end = if selection.is_point() { cursor } else { selection.to() };

		let (raw_text_edit, raw_text) = normalize_completion_edit(&item);
		let (insert_text, snippet) = match item.insert_text_format {
			Some(InsertTextFormat::SNIPPET) => parse_snippet_template(&raw_text)
				.map(|template| {
					let rendered = render_with_resolver(&template, &resolver);
					(rendered.text.clone(), Some(rendered))
				})
				.unwrap_or((raw_text.clone(), None)),
			_ => (raw_text.clone(), None),
		};

		let (mut edits, base_start) = match completion_text_edit(&rope, encoding, raw_text_edit, replace_start, replace_end, &insert_text) {
			Ok(result) => result,
			Err(err) => {
				self.notify(keys::error(err));
				return;
			}
		};

		if let Some(additional) = item.additional_text_edits {
			for edit in additional {
				let planned = convert_text_edit(&rope, encoding, &edit).ok_or_else(|| ApplyError::RangeConversionFailed(buffer_id.0.to_string()));
				match planned {
					Ok(planned) => edits.push(planned),
					Err(err) => {
						let err_msg: String = err.to_string();
						self.notify(keys::error(err_msg));
						return;
					}
				}
			}
		}

		if let Err(err) = validate_non_overlapping(&mut edits, buffer_id) {
			let err_msg: String = err.to_string();
			self.notify(keys::error(err_msg));
			return;
		}

		let plan = BufferEditPlan {
			buffer_id,
			edits,
			opened_temporarily: false,
		};

		let tx = match self.apply_buffer_edit_plan(&plan) {
			Ok(tx) => tx,
			Err(err) => {
				let err_msg: String = err.to_string();
				self.notify(keys::error(err_msg));
				return;
			}
		};

		self.flush_lsp_sync_now(&[buffer_id]);
		let mapped_start = tx.changes().map_pos(base_start, Bias::Left);

		if let Some(snippet) = snippet
			&& self.begin_snippet_session(buffer_id, mapped_start, &snippet)
		{
			if let Some(command) = command {
				self.execute_lsp_command(buffer_id, command.command, command.arguments).await;
			}
			return;
		}

		let new_cursor = mapped_start.saturating_add(insert_text.chars().count());
		if let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(buffer_id) {
			buffer.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		}
		if let Some(command) = command {
			self.execute_lsp_command(buffer_id, command.command, command.arguments).await;
		}
	}
}

fn completion_trigger_kind(trigger: &CompletionTrigger, trigger_char: Option<char>) -> CompletionTriggerKind {
	match trigger {
		CompletionTrigger::Typing if trigger_char.is_some() => CompletionTriggerKind::TRIGGER_CHARACTER,
		CompletionTrigger::Manual | CompletionTrigger::Typing => CompletionTriggerKind::INVOKED,
	}
}

fn completion_replace_start(buffer: &crate::buffer::Buffer) -> usize {
	buffer.with_doc(|doc| completion_replace_start_at(doc.content(), buffer.cursor))
}

pub(super) fn completion_replace_start_at(rope: &xeno_primitives::Rope, cursor: CharIdx) -> usize {
	let mut pos = cursor.min(rope.len_chars());
	while pos > 0 {
		let ch = rope.char(pos - 1);
		if is_completion_word_char(ch) {
			pos = pos.saturating_sub(1);
		} else {
			break;
		}
	}
	pos
}

fn is_completion_word_char(ch: char) -> bool {
	ch.is_alphanumeric() || ch == '_'
}

/// Common LSP trigger characters that cause immediate popup and clear suppression.
pub(super) fn is_completion_trigger_char(ch: char) -> bool {
	matches!(ch, '.' | ':' | '>' | '/' | '@' | '<')
}

fn normalize_completion_edit(item: &CompletionItem) -> (Option<TextEdit>, String) {
	let text_edit = match item.text_edit.clone() {
		Some(CompletionTextEdit::Edit(edit)) => Some(edit),
		Some(CompletionTextEdit::InsertAndReplace(edit)) => Some(TextEdit {
			range: edit.replace,
			new_text: edit.new_text,
		}),
		None => None,
	};

	let raw_text = text_edit
		.as_ref()
		.map(|edit| edit.new_text.clone())
		.or_else(|| item.insert_text.clone())
		.unwrap_or_else(|| item.label.clone());

	(text_edit, raw_text)
}

fn completion_text_edit(
	rope: &xeno_primitives::Rope,
	encoding: OffsetEncoding,
	text_edit: Option<TextEdit>,
	replace_start: usize,
	replace_end: CharIdx,
	insert_text: &str,
) -> Result<(Vec<PlannedTextEdit>, CharIdx), String> {
	let mut edits = Vec::new();
	let base_start = if let Some(mut edit) = text_edit {
		edit.new_text = insert_text.to_string();
		let planned = convert_text_edit(rope, encoding, &edit).ok_or_else(|| "Failed to convert completion textEdit".to_string())?;
		let base_start = planned.range.start;
		edits.push(planned);
		base_start
	} else {
		let start = replace_start.min(replace_end);
		let end = replace_end.max(replace_start);
		edits.push(PlannedTextEdit {
			range: start..end,
			replacement: insert_text.into(),
		});
		start
	};

	Ok((edits, base_start))
}

fn validate_non_overlapping(edits: &mut [PlannedTextEdit], buffer_id: ViewId) -> Result<(), ApplyError> {
	edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
	for window in edits.windows(2) {
		let prev = &window[0];
		let next = &window[1];
		if next.range.start < prev.range.end {
			return Err(ApplyError::OverlappingEdits(buffer_id.0.to_string()));
		}
	}
	Ok(())
}
