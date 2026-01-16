use std::collections::BTreeMap;
use std::ops::Range as StdRange;

use termina::event::{KeyCode, KeyEvent, Modifiers};
use tokio_util::sync::CancellationToken;
use xeno_lsp::lsp_types::{
	CodeActionOrCommand, Command, CompletionItem, CompletionTextEdit, CompletionTriggerKind,
	Diagnostic, Documentation, InsertTextFormat, MarkupContent, SignatureHelp, TextEdit,
};
use xeno_lsp::{OffsetEncoding, char_range_to_lsp_range, lsp_range_to_char_range};
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::Bias;
use xeno_primitives::{Range, Selection};
use xeno_registry_notifications::keys;

use crate::buffer::BufferId;
use crate::impls::Editor;
use super::completion_controller::{CompletionRequest, CompletionTrigger};
use super::completion_filter::{extract_query, filter_items};
use super::events::map_completion_item_with_indices;
use super::snippet::{Snippet, SnippetPlaceholder, parse_snippet};
use super::workspace_edit::{ApplyError, BufferEditPlan, PlannedTextEdit, convert_text_edit};
use super::types::{LspMenuKind, LspMenuState};
use crate::completion::{CompletionState, SelectionIntent};
use crate::info_popup::PopupAnchor;
use crate::{CompletionItem as UiCompletionItem, CompletionKind};

impl Editor {
	pub(crate) async fn handle_lsp_menu_key(&mut self, key: &KeyEvent) -> bool {
		let menu_kind = self
			.overlays
			.get::<LspMenuState>()
			.and_then(|state| state.active())
			.cloned();
		let Some(menu_kind) = menu_kind else {
			return false;
		};

		let buffer_id = match &menu_kind {
			LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
			LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
		};
		if buffer_id != self.focused_view() {
			self.clear_lsp_menu();
			return false;
		}

		match key.code {
			KeyCode::Escape => {
				self.completion_controller.cancel();
				if self
					.overlays
					.get::<CompletionState>()
					.is_some_and(|s| s.active)
				{
					self.overlays.get_or_default::<CompletionState>().suppressed = true;
				}
				self.clear_lsp_menu();
				return true;
			}
			KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
				self.move_lsp_menu_selection(-1);
				return true;
			}
			KeyCode::Down | KeyCode::Char('j') => {
				self.move_lsp_menu_selection(1);
				return true;
			}
			KeyCode::PageUp => {
				self.page_lsp_menu_selection(-1);
				return true;
			}
			KeyCode::PageDown => {
				self.page_lsp_menu_selection(1);
				return true;
			}
			KeyCode::Tab => {
				let selected_idx = self
					.overlays
					.get::<CompletionState>()
					.and_then(|state| state.selected_idx);
				if let Some(idx) = selected_idx {
					self.completion_controller.cancel();
					self.clear_lsp_menu();
					match menu_kind {
						LspMenuKind::Completion { buffer_id, items } => {
							if let Some(item) = items.get(idx).cloned() {
								self.apply_completion_item(buffer_id, item).await;
							}
						}
						LspMenuKind::CodeAction { buffer_id, actions } => {
							if let Some(action) = actions.get(idx).cloned() {
								self.apply_code_action_or_command(buffer_id, action).await;
							}
						}
					}
				} else {
					let state = self.overlays.get_or_default::<CompletionState>();
					if !state.items.is_empty() {
						state.selected_idx = Some(0);
						state.selection_intent = SelectionIntent::Manual;
						state.ensure_selected_visible();
						self.frame.needs_redraw = true;
					}
				}
				return true;
			}
			KeyCode::Char('y') if key.modifiers.contains(Modifiers::CONTROL) => {
				let state = self.overlays.get::<CompletionState>();
				let idx = state
					.and_then(|s| s.selected_idx)
					.or_else(|| state.filter(|s| !s.items.is_empty()).map(|_| 0));
				self.completion_controller.cancel();
				self.clear_lsp_menu();
				if let Some(idx) = idx {
					match menu_kind {
						LspMenuKind::Completion { buffer_id, items } => {
							if let Some(item) = items.get(idx).cloned() {
								self.apply_completion_item(buffer_id, item).await;
							}
						}
						LspMenuKind::CodeAction { buffer_id, actions } => {
							if let Some(action) = actions.get(idx).cloned() {
								self.apply_code_action_or_command(buffer_id, action).await;
							}
						}
					}
				}
				return true;
			}
			KeyCode::Enter => return false,
			_ => {}
		}

		false
	}

	pub(crate) fn is_completion_trigger_key(&self, key: &KeyEvent) -> bool {
		key.code == KeyCode::Char(' ')
			&& key.modifiers.contains(Modifiers::CONTROL)
			&& !key.modifiers.contains(Modifiers::ALT)
			&& !key.modifiers.contains(Modifiers::SHIFT)
	}

	pub(crate) fn trigger_lsp_completion(
		&mut self,
		trigger: CompletionTrigger,
		trigger_char: Option<char>,
	) {
		let is_trigger_char = trigger_char.is_some_and(is_completion_trigger_char);
		let is_manual = matches!(trigger, CompletionTrigger::Manual);

		if is_trigger_char || is_manual {
			self.overlays.get_or_default::<CompletionState>().suppressed = false;
		} else if self
			.overlays
			.get::<CompletionState>()
			.is_some_and(|s| s.suppressed)
		{
			return;
		}

		let buffer = self.buffer();
		if buffer.mode() != xeno_primitives::Mode::Insert {
			return;
		}
		if buffer.path().is_none() || buffer.file_type().is_none() {
			return;
		}

		let Some((client, uri, position)) =
			self.lsp.prepare_position_request(buffer).ok().flatten()
		else {
			return;
		};
		if !client.supports_completion() {
			return;
		}

		let selection = buffer.selection.primary();
		let replace_start = if selection.is_empty() {
			completion_replace_start(buffer)
		} else {
			selection.from()
		};
		let request = CompletionRequest {
			buffer_id: self.focused_view(),
			cursor: buffer.cursor,
			doc_version: buffer.version(),
			replace_start,
			client,
			uri,
			position,
			debounce: trigger.debounce(),
			ui_tx: self.lsp_ui_tx.clone(),
			trigger_kind: completion_trigger_kind(&trigger, trigger_char),
			trigger_character: trigger_char.map(|c| c.to_string()),
		};

		self.completion_controller.trigger(request);
	}

	/// Refilters the active completion menu with the current query.
	///
	/// Called when the user types or deletes while a completion menu is visible,
	/// to update filtering without waiting for a new LSP response.
	pub(crate) fn refilter_completion(&mut self) {
		let menu_kind = self.overlays.get::<LspMenuState>().and_then(|s| s.active());
		let Some(LspMenuKind::Completion { buffer_id, items }) = menu_kind else {
			return;
		};
		let buffer_id = *buffer_id;
		let items = items.clone();

		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};

		let replace_start = self
			.overlays
			.get::<CompletionState>()
			.map(|s| s.replace_start)
			.unwrap_or(0);

		if buffer.cursor < replace_start {
			self.clear_lsp_menu();
			return;
		}

		let query =
			buffer.with_doc(|doc| extract_query(doc.content(), replace_start, buffer.cursor));
		let filtered = filter_items(&items, &query);

		if filtered.is_empty() {
			self.clear_lsp_menu();
			return;
		}

		let display_items: Vec<UiCompletionItem> = filtered
			.iter()
			.map(|f| map_completion_item_with_indices(&items[f.index], f.match_indices.clone()))
			.collect();

		let completions = self.overlays.get_or_default::<CompletionState>();
		completions.items = display_items;
		completions.selected_idx = None;
		completions.selection_intent = SelectionIntent::Auto;
		completions.scroll_offset = 0;
		completions.query = query;

		self.frame.needs_redraw = true;
	}

	pub(crate) async fn open_code_action_menu(&mut self) -> bool {
		let buffer_id = self.focused_view();
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return false;
		};
		let Some((client, uri, _)) = self.lsp.prepare_position_request(buffer).ok().flatten()
		else {
			return false;
		};
		if !client.supports_code_action() {
			self.notify(keys::warn::call(
				"Code actions not supported for this buffer",
			));
			return false;
		}

		let selection = buffer.selection.primary();
		let start = if selection.is_empty() {
			buffer.cursor
		} else {
			selection.from()
		};
		let end = if selection.is_empty() {
			buffer.cursor
		} else {
			selection.to()
		};
		let encoding = client.offset_encoding();
		let Some(range) =
			buffer.with_doc(|doc| char_range_to_lsp_range(doc.content(), start, end, encoding))
		else {
			self.notify(keys::error::call("Invalid range for code actions"));
			return false;
		};
		let diagnostics = buffer.with_doc(|doc| {
			diagnostics_for_range(
				&self.lsp.get_diagnostics(buffer),
				doc.content(),
				encoding,
				start..end,
			)
		});
		let context = xeno_lsp::lsp_types::CodeActionContext {
			diagnostics,
			only: None,
			trigger_kind: None,
		};
		let actions = match client.code_action(uri, range, context).await {
			Ok(Some(actions)) => actions,
			Ok(None) => Vec::new(),
			Err(err) => {
				self.notify(keys::error::call(err.to_string()));
				return false;
			}
		};

		let actions = actions
			.into_iter()
			.filter(|action| match action {
				CodeActionOrCommand::CodeAction(action) => action.disabled.is_none(),
				CodeActionOrCommand::Command(_) => true,
			})
			.collect::<Vec<_>>();

		if actions.is_empty() {
			self.notify(keys::info::call("No code actions available"));
			return false;
		}

		let display_items: Vec<UiCompletionItem> =
			actions.iter().map(map_code_action_item).collect();

		let completions = self.overlays.get_or_default::<CompletionState>();
		completions.items = display_items;
		completions.selected_idx = Some(0);
		completions.active = true;
		completions.replace_start = 0;
		completions.scroll_offset = 0;

		let menu_state = self.overlays.get_or_default::<LspMenuState>();
		menu_state.set(LspMenuKind::CodeAction { buffer_id, actions });

		self.frame.needs_redraw = true;
		true
	}

	pub(crate) fn trigger_signature_help(&mut self) {
		let buffer_id = self.focused_view();
		let (client, uri, position, cursor, doc_version) = {
			let buffer = self.buffer();
			if buffer.mode() != xeno_primitives::Mode::Insert {
				return;
			}
			let Some((client, uri, position)) =
				self.lsp.prepare_position_request(buffer).ok().flatten()
			else {
				return;
			};
			if !client.supports_signature_help() {
				return;
			}
			(client, uri, position, buffer.cursor, buffer.version())
		};

		self.cancel_signature_help();
		self.signature_help_generation = self.signature_help_generation.wrapping_add(1);
		let generation = self.signature_help_generation;

		let cancel = CancellationToken::new();
		self.signature_help_cancel = Some(cancel.clone());

		let anchor = signature_help_anchor(self, buffer_id);
		let ui_tx = self.lsp_ui_tx.clone();

		tokio::spawn(async move {
			let help = tokio::select! {
				_ = cancel.cancelled() => return,
				result = client.signature_help(uri, position) => result,
			};

			if cancel.is_cancelled() {
				return;
			}

			let help = match help {
				Ok(Some(help)) => help,
				_ => return,
			};

			let contents = format_signature_help(&help);
			if contents.is_empty() {
				return;
			}

			let _ = ui_tx.send(super::events::LspUiEvent::SignatureHelp {
				generation,
				buffer_id,
				cursor,
				doc_version,
				contents,
				anchor,
			});
		});
	}

	pub(crate) fn cancel_signature_help(&mut self) {
		if let Some(cancel) = self.signature_help_cancel.take() {
			cancel.cancel();
		}
	}

	fn move_lsp_menu_selection(&mut self, delta: isize) {
		let state = self.overlays.get_or_default::<CompletionState>();
		if state.items.is_empty() {
			return;
		}
		let total = state.items.len();
		let current = state.selected_idx.unwrap_or(0) as isize;
		let mut next = current + delta;
		if next < 0 {
			next = total as isize - 1;
		} else if next as usize >= total {
			next = 0;
		}
		state.selected_idx = Some(next as usize);
		state.selection_intent = SelectionIntent::Manual;
		state.ensure_selected_visible();
		self.frame.needs_redraw = true;
	}

	fn page_lsp_menu_selection(&mut self, direction: isize) {
		let state = self.overlays.get_or_default::<CompletionState>();
		if state.items.is_empty() {
			return;
		}
		let step = CompletionState::MAX_VISIBLE as isize;
		let delta = if direction >= 0 { step } else { -step };
		let total = state.items.len();
		let current = state.selected_idx.unwrap_or(0) as isize;
		let mut next = current + delta;
		if next < 0 {
			next = 0;
		} else if next as usize >= total {
			next = total.saturating_sub(1) as isize;
		}
		state.selected_idx = Some(next as usize);
		state.selection_intent = SelectionIntent::Manual;
		state.ensure_selected_visible();
		self.frame.needs_redraw = true;
	}

	async fn apply_completion_item(&mut self, buffer_id: BufferId, item: CompletionItem) {
		let (encoding, selection, cursor, rope, readonly) = {
			let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
				return;
			};
			(
				self.lsp.offset_encoding_for_buffer(buffer),
				buffer.selection.primary(),
				buffer.cursor,
				buffer.with_doc(|doc| doc.content().clone()),
				buffer.is_readonly(),
			)
		};
		if readonly {
			self.notify(keys::buffer_readonly);
			return;
		}
		let command = item.command.clone();

		let replace_start = if selection.is_empty() {
			self.overlays
				.get::<CompletionState>()
				.map(|state| state.replace_start)
				.unwrap_or_else(|| completion_replace_start_at(&rope, cursor))
		} else {
			selection.from()
		};
		let replace_end = if selection.is_empty() {
			cursor
		} else {
			selection.to()
		};

		let (raw_text_edit, raw_text) = normalize_completion_edit(&item);
		let (insert_text, snippet) = match item.insert_text_format {
			Some(InsertTextFormat::SNIPPET) => parse_snippet(&raw_text)
				.map(|parsed| (parsed.text.clone(), Some(parsed)))
				.unwrap_or((raw_text.clone(), None)),
			_ => (raw_text.clone(), None),
		};

		let (mut edits, base_start) = match completion_text_edit(
			&rope,
			encoding,
			raw_text_edit,
			replace_start,
			replace_end,
			&insert_text,
		) {
			Ok(result) => result,
			Err(err) => {
				self.notify(keys::error::call(err));
				return;
			}
		};

		if let Some(additional) = item.additional_text_edits {
			for edit in additional {
				let planned = convert_text_edit(&rope, encoding, &edit)
					.ok_or_else(|| ApplyError::RangeConversionFailed(buffer_id.0.to_string()));
				match planned {
					Ok(planned) => edits.push(planned),
					Err(err) => {
						self.notify(keys::error::call(err.to_string()));
						return;
					}
				}
			}
		}

		if let Err(err) = validate_non_overlapping(&mut edits, buffer_id) {
			self.notify(keys::error::call(err.to_string()));
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
				self.notify(keys::error::call(err.to_string()));
				return;
			}
		};

		self.flush_lsp_sync_now(&[buffer_id]);

		if let Some(selection) = completion_snippet_selection(&tx, base_start, snippet) {
			if let Some(buffer) = self.core.buffers.get_buffer_mut(buffer_id) {
				let cursor = selection.primary().head;
				buffer.set_cursor_and_selection(cursor, selection);
			}
			if let Some(command) = command {
				self.execute_lsp_command(buffer_id, command).await;
			}
			return;
		}

		let new_cursor = tx
			.changes()
			.map_pos(base_start, Bias::Left)
			.saturating_add(insert_text.chars().count());
		if let Some(buffer) = self.core.buffers.get_buffer_mut(buffer_id) {
			buffer.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		}
		if let Some(command) = command {
			self.execute_lsp_command(buffer_id, command).await;
		}
	}

	async fn apply_code_action_or_command(
		&mut self,
		buffer_id: BufferId,
		action: CodeActionOrCommand,
	) {
		match action {
			CodeActionOrCommand::Command(command) => {
				self.execute_lsp_command(buffer_id, command).await;
			}
			CodeActionOrCommand::CodeAction(action) => {
				if let Some(disabled) = action.disabled {
					self.notify(keys::warn::call(disabled.reason));
					return;
				}
				if let Some(edit) = action.edit
					&& let Err(err) = self.apply_workspace_edit(edit).await
				{
					self.notify(keys::error::call(err.to_string()));
					return;
				}
				if let Some(command) = action.command {
					self.execute_lsp_command(buffer_id, command).await;
				}
			}
		}
	}

	async fn execute_lsp_command(&mut self, buffer_id: BufferId, command: Command) {
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let Some((client, _, _)) = self.lsp.prepare_position_request(buffer).ok().flatten() else {
			self.notify(keys::error::call("LSP client unavailable for command"));
			return;
		};

		match client
			.execute_command(command.command, command.arguments)
			.await
		{
			Ok(Some(_)) | Ok(None) => {}
			Err(err) => {
				self.notify(keys::error::call(err.to_string()));
			}
		}
	}
}

fn completion_trigger_kind(
	trigger: &CompletionTrigger,
	trigger_char: Option<char>,
) -> CompletionTriggerKind {
	match trigger {
		CompletionTrigger::Typing if trigger_char.is_some() => {
			CompletionTriggerKind::TRIGGER_CHARACTER
		}
		CompletionTrigger::Manual | CompletionTrigger::Typing => CompletionTriggerKind::INVOKED,
	}
}

fn map_code_action_item(action: &CodeActionOrCommand) -> UiCompletionItem {
	let (label, detail) = match action {
		CodeActionOrCommand::Command(command) => (command.title.clone(), None),
		CodeActionOrCommand::CodeAction(action) => (
			action.title.clone(),
			action.kind.as_ref().map(|kind| kind.as_str().to_string()),
		),
	};

	UiCompletionItem {
		label: label.clone(),
		insert_text: label,
		detail,
		filter_text: None,
		kind: CompletionKind::Command,
		match_indices: None,
	}
}

fn diagnostics_for_range(
	diagnostics: &[Diagnostic],
	rope: &xeno_primitives::Rope,
	encoding: OffsetEncoding,
	selection: StdRange<CharIdx>,
) -> Vec<Diagnostic> {
	let mut out = Vec::new();
	for diag in diagnostics {
		let Some((start, end)) = lsp_range_to_char_range(rope, diag.range, encoding) else {
			continue;
		};
		let diag_range = start..end;
		let includes = if selection.start == selection.end {
			diag_range.start <= selection.start && selection.start <= diag_range.end
		} else {
			ranges_overlap(selection.clone(), diag_range)
		};
		if includes {
			out.push(diag.clone());
		}
	}
	out
}

fn ranges_overlap(a: StdRange<CharIdx>, b: StdRange<CharIdx>) -> bool {
	a.start < b.end && b.start < a.end
}

fn signature_help_anchor(editor: &Editor, buffer_id: BufferId) -> PopupAnchor {
	let Some(buffer) = editor.get_buffer(buffer_id) else {
		return PopupAnchor::Center;
	};
	let tab_width = editor.tab_width_for(buffer_id);
	let Some((row, col)) = buffer.doc_to_screen_position(buffer.cursor, tab_width) else {
		return PopupAnchor::Center;
	};
	let view_area = editor.focused_view_area();
	let x = view_area.x.saturating_add(col);
	let y = view_area.y.saturating_add(row.saturating_add(1));
	PopupAnchor::Point { x, y }
}

fn format_signature_help(help: &SignatureHelp) -> String {
	let signature = help
		.active_signature
		.and_then(|idx| help.signatures.get(idx as usize))
		.or_else(|| help.signatures.first());
	let Some(signature) = signature else {
		return String::new();
	};

	let mut output = signature.label.clone();
	if let Some(doc) = signature.documentation.as_ref() {
		let doc = format_documentation(doc);
		if !doc.is_empty() {
			output.push_str("\n\n");
			output.push_str(&doc);
		}
	}

	output
}

fn format_documentation(doc: &Documentation) -> String {
	match doc {
		Documentation::String(text) => text.clone(),
		Documentation::MarkupContent(MarkupContent { value, .. }) => value.clone(),
	}
}

fn completion_replace_start(buffer: &crate::buffer::Buffer) -> usize {
	buffer.with_doc(|doc| completion_replace_start_at(doc.content(), buffer.cursor))
}

fn completion_replace_start_at(rope: &xeno_primitives::Rope, cursor: CharIdx) -> usize {
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
fn is_completion_trigger_char(ch: char) -> bool {
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
		let planned = convert_text_edit(rope, encoding, &edit)
			.ok_or_else(|| "Failed to convert completion textEdit".to_string())?;
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

fn validate_non_overlapping(
	edits: &mut [PlannedTextEdit],
	buffer_id: BufferId,
) -> Result<(), ApplyError> {
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

fn completion_snippet_selection(
	tx: &xeno_primitives::Transaction,
	base_start: CharIdx,
	snippet: Option<Snippet>,
) -> Option<Selection> {
	let snippet = snippet?;
	if snippet.placeholders.is_empty() {
		return None;
	}

	let mapped_start = tx.changes().map_pos(base_start, Bias::Left);
	let mut by_index: BTreeMap<u32, Vec<StdRange<CharIdx>>> = BTreeMap::new();
	for SnippetPlaceholder { index, range } in snippet.placeholders {
		let start = mapped_start.saturating_add(range.start);
		let end = mapped_start.saturating_add(range.end);
		by_index.entry(index).or_default().push(start..end);
	}

	let mut selection_ranges = by_index
		.range(1..)
		.next()
		.map(|(_, ranges)| ranges.clone())
		.or_else(|| by_index.get(&0).cloned())?;

	if selection_ranges.is_empty() {
		return None;
	}

	let primary = selection_ranges.remove(0);
	let selection = Selection::new(
		Range::new(primary.start, primary.end),
		selection_ranges
			.into_iter()
			.map(|range| Range::new(range.start, range.end)),
	);
	Some(selection)
}
