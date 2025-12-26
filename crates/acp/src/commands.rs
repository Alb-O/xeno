//! ACP commands registered via distributed_slice.
//!
//! These commands provide the user interface for the ACP integration:
//! - acp_start: Start the agent
//! - acp_stop: Stop the agent
//! - acp_toggle: Toggle the chat panel
//! - acp_insert_last: Insert the last assistant response
//! - acp_cancel: Cancel the current request

use std::path::PathBuf;

use futures::future::LocalBoxFuture;
use tome_manifest::{CommandContext, CommandError, CommandOutcome};
use tome_stdlib::{NotifyINFOExt, command};

use crate::AcpManager;
use crate::panel::{AcpChatPanel, chat_panel_ui_id};
use crate::types::ChatPanelState;

/// Panel ID for the ACP chat panel.
pub const ACP_PANEL_ID: u64 = u64::MAX - 1;

command!(acp_start, {
	aliases: &["acp.start"],
	description: "Start the ACP agent"
}, handler: cmd_acp_start);

fn cmd_acp_start<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		// TODO: Get cwd from editor path when path() is added to EditorOps
		let cwd = std::env::current_dir()
			.ok()
			.unwrap_or_else(|| PathBuf::from("."));

		let cwd = cwd.canonicalize().unwrap_or(cwd);

		let editor = ctx.require_editor_mut();
		if let Some(acp) = editor.extensions.get_mut::<AcpManager>() {
			acp.start(cwd);
			ctx.info("ACP agent starting...");
			Ok(CommandOutcome::Ok)
		} else {
			Err(CommandError::Failed("ACP extension not loaded".to_string()))
		}
	})
}

command!(acp_stop, {
	aliases: &["acp.stop"],
	description: "Stop the ACP agent"
}, handler: cmd_acp_stop);

fn cmd_acp_stop<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let editor = ctx.require_editor_mut();
		if let Some(acp) = editor.extensions.get_mut::<AcpManager>() {
			acp.stop();
			ctx.info("ACP agent stopped");
			Ok(CommandOutcome::Ok)
		} else {
			Err(CommandError::Failed("ACP extension not loaded".to_string()))
		}
	})
}

command!(acp_toggle, {
	aliases: &["acp.toggle", "acp"],
	description: "Toggle the ACP chat panel"
}, handler: cmd_acp_toggle);

fn cmd_acp_toggle<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let editor = ctx.require_editor_mut();
		let (panel_id, ui_id, has_acp) =
			if let Some(acp) = editor.extensions.get_mut::<AcpManager>() {
				let panel_id = acp.panel_id().unwrap_or(ACP_PANEL_ID);
				let mut panels = acp.state.panels.lock();
				if let std::collections::hash_map::Entry::Vacant(e) = panels.entry(panel_id) {
					e.insert(ChatPanelState::new("ACP Agent".to_string()));
					acp.set_panel_id(Some(panel_id));
					(panel_id, chat_panel_ui_id(panel_id), true)
				} else {
					(panel_id, chat_panel_ui_id(panel_id), false)
				}
			} else {
				return Err(CommandError::Failed("ACP extension not loaded".to_string()));
			};

		if has_acp {
			editor.ui.register_panel(Box::new(AcpChatPanel::new(
				panel_id,
				"ACP Agent".to_string(),
			)));
		}

		editor.ui.toggle_panel(&ui_id);
		editor.needs_redraw = true;
		Ok(CommandOutcome::Ok)
	})
}

command!(acp_insert_last, {
	aliases: &["acp.insert_last"],
	description: "Insert the last ACP assistant response"
}, handler: cmd_acp_insert_last);

fn cmd_acp_insert_last<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let editor = ctx.require_editor_mut();
		if let Some(acp) = editor.extensions.get::<AcpManager>() {
			let text = acp.last_assistant_text();
			if text.is_empty() {
				return Err(CommandError::Failed(
					"No assistant response available".to_string(),
				));
			}
			editor.insert_text(&text);
			Ok(CommandOutcome::Ok)
		} else {
			Err(CommandError::Failed("ACP extension not loaded".to_string()))
		}
	})
}

command!(acp_cancel, {
	aliases: &["acp.cancel"],
	description: "Cancel the current ACP request"
}, handler: cmd_acp_cancel);

fn cmd_acp_cancel<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let editor = ctx.require_editor_mut();
		if let Some(acp) = editor.extensions.get_mut::<AcpManager>() {
			acp.cancel();
			ctx.info("ACP request cancelled");
			Ok(CommandOutcome::Ok)
		} else {
			Err(CommandError::Failed("ACP extension not loaded".to_string()))
		}
	})
}

trait CommandContextExt {
	fn require_editor_mut(&mut self) -> &mut tome_api::editor::Editor;
}

impl<'a> CommandContextExt for CommandContext<'a> {
	fn require_editor_mut(&mut self) -> &mut tome_api::editor::Editor {
		// SAFETY: We know that in tome-term, EditorOps is implemented by Editor
		unsafe {
			&mut *(self.editor as *mut dyn tome_manifest::EditorOps
				as *mut tome_api::editor::Editor)
		}
	}
}
