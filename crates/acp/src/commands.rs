//! ACP commands registered via distributed_slice.
//!
//! These commands provide the user interface for the ACP integration:
//! - acp_start: Start the agent
//! - acp_stop: Stop the agent
//! - acp_insert_last: Insert the last assistant response
//! - acp_cancel: Cancel the current request

use std::path::PathBuf;

use futures::future::LocalBoxFuture;
use evildoer_manifest::{CommandContext, CommandError, CommandOutcome};
use evildoer_stdlib::{NotifyINFOExt, command};

use crate::AcpManager;

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

command!(acp_model, {
	aliases: &["acp.model"],
	description: "Set the ACP model (e.g., acp.model anthropic/claude-sonnet-4)"
}, handler: cmd_acp_model);

fn cmd_acp_model<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let args = ctx.args;
		let editor = ctx.require_editor_mut();
		if let Some(acp) = editor.extensions.get_mut::<AcpManager>() {
			if let Some(model_id) = args.first() {
				acp.set_model(model_id.to_string());
				ctx.info(&format!("Setting model to: {}", model_id));
				Ok(CommandOutcome::Ok)
			} else {
				// No argument - show current model and available models
				let current = acp.current_model();
				let available = acp.available_models();
				if available.is_empty() {
					ctx.info(&format!("Current model: {}", current));
				} else {
					let models: Vec<String> = available
						.iter()
						.map(|m| format!("  {} ({})", m.model_id, m.name))
						.collect();
					ctx.info(&format!(
						"Current model: {}\nAvailable models:\n{}",
						current,
						models.join("\n")
					));
				}
				Ok(CommandOutcome::Ok)
			}
		} else {
			Err(CommandError::Failed("ACP extension not loaded".to_string()))
		}
	})
}

trait CommandContextExt {
	fn require_editor_mut(&mut self) -> &mut evildoer_api::editor::Editor;
}

impl<'a> CommandContextExt for CommandContext<'a> {
	fn require_editor_mut(&mut self) -> &mut evildoer_api::editor::Editor {
		// SAFETY: We know that in evildoer-term, EditorOps is implemented by Editor
		unsafe {
			&mut *(self.editor as *mut dyn evildoer_manifest::EditorOps
				as *mut evildoer_api::editor::Editor)
		}
	}
}
