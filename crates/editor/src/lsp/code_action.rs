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

use xeno_lsp::lsp_types::CodeActionOrCommand;
use xeno_primitives::range::CharIdx;
use xeno_registry::notifications::keys;

use super::types::{LspMenuKind, LspMenuState};
use crate::buffer::ViewId;
use crate::completion::CompletionState;
use crate::lsp::api::{Diagnostic, DiagnosticSeverity};
use crate::{CompletionItem as UiCompletionItem, CompletionKind, Editor};

impl Editor {
	pub(crate) async fn open_code_action_menu(&mut self) -> bool {
		let buffer_id = self.focused_view();
		let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
			return false;
		};
		let Some((client, uri, _)) = self.state.lsp.prepare_position_request(buffer).ok().flatten() else {
			return false;
		};

		if !client.supports_code_action() {
			self.notify(keys::warn("Code actions not supported for this buffer"));
			return false;
		}

		let selection = buffer.selection.primary();
		let start = if selection.is_point() { buffer.cursor } else { selection.from() };
		let end = if selection.is_point() { buffer.cursor } else { selection.to() };
		let encoding = client.offset_encoding();
		let Some(range) = buffer.with_doc(|doc| xeno_lsp::char_range_to_lsp_range(doc.content(), start, end, encoding)) else {
			self.notify(keys::error("Invalid range for code actions"));
			return false;
		};
		let diagnostics = buffer.with_doc(|doc| diagnostics_for_range(&self.state.lsp.get_diagnostics(buffer), doc.content(), start..end));
		let lsp_diagnostics: Vec<xeno_lsp::lsp_types::Diagnostic> = diagnostics
			.into_iter()
			.map(|d| xeno_lsp::lsp_types::Diagnostic {
				range: xeno_lsp::lsp_types::Range {
					start: xeno_lsp::lsp_types::Position {
						line: d.range.0 as u32,
						character: d.range.1 as u32,
					},
					end: xeno_lsp::lsp_types::Position {
						line: d.range.2 as u32,
						character: d.range.3 as u32,
					},
				},
				severity: Some(match d.severity {
					DiagnosticSeverity::Error => xeno_lsp::lsp_types::DiagnosticSeverity::ERROR,
					DiagnosticSeverity::Warning => xeno_lsp::lsp_types::DiagnosticSeverity::WARNING,
					DiagnosticSeverity::Info => xeno_lsp::lsp_types::DiagnosticSeverity::INFORMATION,
					DiagnosticSeverity::Hint => xeno_lsp::lsp_types::DiagnosticSeverity::HINT,
				}),
				code: d.code.map(xeno_lsp::lsp_types::NumberOrString::String),
				source: d.source,
				message: d.message,
				..Default::default()
			})
			.collect();

		let context = xeno_lsp::lsp_types::CodeActionContext {
			diagnostics: lsp_diagnostics,
			only: None,
			trigger_kind: None,
		};
		let actions = match client.code_action(uri, range, context).await {
			Ok(Some(actions)) => actions,
			Ok(None) => Vec::new(),
			Err(err) => {
				let err_msg: String = err.to_string();
				self.notify(keys::error(err_msg));
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

		let display_items: Vec<UiCompletionItem> = actions.iter().map(map_code_action_item).collect();

		let completions = self.overlays_mut().get_or_default::<CompletionState>();
		completions.items = display_items;
		completions.selected_idx = Some(0);
		completions.active = true;
		completions.replace_start = 0;
		completions.scroll_offset = 0;

		let menu_state = self.overlays_mut().get_or_default::<LspMenuState>();
		menu_state.set(LspMenuKind::CodeAction { buffer_id, actions });

		self.state.frame.needs_redraw = true;
		true
	}

	pub(crate) async fn apply_code_action_or_command(&mut self, buffer_id: ViewId, action: CodeActionOrCommand) {
		match action {
			CodeActionOrCommand::Command(command) => {
				self.execute_lsp_command(buffer_id, command.command, command.arguments).await;
			}
			CodeActionOrCommand::CodeAction(action) => {
				if let Some(disabled) = action.disabled {
					self.notify(keys::warn(disabled.reason));
					return;
				}
				if let Some(edit) = action.edit
					&& let Err(err) = self.apply_workspace_edit(edit).await
				{
					let err_msg: String = err.to_string();
					self.notify(keys::error(err_msg));
					return;
				}
				if let Some(command) = action.command {
					self.execute_lsp_command(buffer_id, command.command, command.arguments).await;
				}
			}
		}
	}

	pub(crate) async fn execute_lsp_command(&mut self, buffer_id: ViewId, command: String, arguments: Option<Vec<serde_json::Value>>) {
		let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let Some((client, _, _)) = self.state.lsp.prepare_position_request(buffer).ok().flatten() else {
			self.notify(keys::error("LSP client unavailable for command"));
			return;
		};

		match client.execute_command(command, arguments).await {
			Ok(Some(_)) | Ok(None) => {}
			Err(err) => {
				let err_msg: String = err.to_string();
				self.notify(keys::error(err_msg));
			}
		}
	}
}

fn map_code_action_item(action: &CodeActionOrCommand) -> UiCompletionItem {
	let (label, detail) = match action {
		CodeActionOrCommand::Command(command) => (command.title.clone(), None),
		CodeActionOrCommand::CodeAction(action) => (action.title.clone(), action.kind.as_ref().map(|kind| kind.as_str().to_string())),
	};

	UiCompletionItem {
		label: label.clone(),
		insert_text: label,
		detail,
		filter_text: None,
		kind: CompletionKind::Command,
		match_indices: None,
		right: None,
	}
}

fn diagnostics_for_range(diagnostics: &[Diagnostic], rope: &xeno_primitives::Rope, selection: StdRange<CharIdx>) -> Vec<Diagnostic> {
	let mut out = Vec::new();
	for diag in diagnostics {
		let (start_line, start_char, end_line, end_char) = diag.range;

		let start = rope.line_to_char(start_line) + start_char;
		let end = rope.line_to_char(end_line) + end_char;

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
