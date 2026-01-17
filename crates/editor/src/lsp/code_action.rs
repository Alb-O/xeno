//! LSP code action menu and execution.
//!
//! Code actions are context-sensitive operations offered by language servers—quick fixes,
//! refactorings, source organization. When the user requests code actions, this module
//! queries the server with the current selection range and any diagnostics overlapping
//! that range, then presents available actions in a menu.
//!
//! Actions come in two forms: workspace edits (applied directly to buffers) and commands
//! (sent back to the server for execution). Some actions include both.

use std::ops::Range as StdRange;

use xeno_lsp::lsp_types::{CodeActionOrCommand, Command, Diagnostic};
use xeno_lsp::{OffsetEncoding, lsp_range_to_char_range};
use xeno_primitives::range::CharIdx;
use xeno_registry_notifications::keys;

use super::types::{LspMenuKind, LspMenuState};
use crate::buffer::ViewId;
use crate::completion::CompletionState;
use crate::impls::Editor;
use crate::{CompletionItem as UiCompletionItem, CompletionKind};

impl Editor {
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
			self.notify(keys::warn("Code actions not supported for this buffer"));
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
		let Some(range) = buffer.with_doc(|doc| {
			xeno_lsp::char_range_to_lsp_range(doc.content(), start, end, encoding)
		}) else {
			self.notify(keys::error("Invalid range for code actions"));
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
				self.notify(keys::error(err.to_string()));
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
			self.notify(keys::info("No code actions available"));
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

	pub(crate) async fn apply_code_action_or_command(
		&mut self,
		buffer_id: ViewId,
		action: CodeActionOrCommand,
	) {
		match action {
			CodeActionOrCommand::Command(command) => {
				self.execute_lsp_command(buffer_id, command).await;
			}
			CodeActionOrCommand::CodeAction(action) => {
				if let Some(disabled) = action.disabled {
					self.notify(keys::warn(disabled.reason));
					return;
				}
				if let Some(edit) = action.edit
					&& let Err(err) = self.apply_workspace_edit(edit).await
				{
					self.notify(keys::error(err.to_string()));
					return;
				}
				if let Some(command) = action.command {
					self.execute_lsp_command(buffer_id, command).await;
				}
			}
		}
	}

	pub(crate) async fn execute_lsp_command(&mut self, buffer_id: ViewId, command: Command) {
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let Some((client, _, _)) = self.lsp.prepare_position_request(buffer).ok().flatten() else {
			self.notify(keys::error("LSP client unavailable for command"));
			return;
		};

		match client
			.execute_command(command.command, command.arguments)
			.await
		{
			Ok(Some(_)) | Ok(None) => {}
			Err(err) => {
				self.notify(keys::error(err.to_string()));
			}
		}
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
