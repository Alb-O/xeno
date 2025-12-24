//! ACP commands registered via distributed_slice.
//!
//! These commands provide the user interface for the ACP integration:
//! - acp_start: Start the agent
//! - acp_stop: Stop the agent
//! - acp_toggle: Toggle the chat panel
//! - acp_insert_last: Insert the last assistant response
//! - acp_cancel: Cancel the current request

use std::path::PathBuf;

use tome_core::command;
use tome_core::ext::{CommandContext, CommandError, CommandOutcome};

command!(acp_start, {
	aliases: &["acp.start"],
	description: "Start the ACP agent"
}, handler: cmd_acp_start);

fn cmd_acp_start(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	// Get the current working directory from the editor's path or fall back to cwd
	let cwd = ctx
		.editor
		.path()
		.and_then(|p| p.parent().map(|p| p.to_path_buf()))
		.or_else(|| std::env::current_dir().ok())
		.unwrap_or_else(|| PathBuf::from("."));

	let cwd = cwd.canonicalize().unwrap_or(cwd);

	// We need to call acp_start on the editor, which will be handled by EditorOps
	ctx.editor.acp_start(cwd)?;
	ctx.message("ACP agent starting...");
	Ok(CommandOutcome::Ok)
}

command!(acp_stop, {
	aliases: &["acp.stop"],
	description: "Stop the ACP agent"
}, handler: cmd_acp_stop);

fn cmd_acp_stop(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.editor.acp_stop()?;
	ctx.message("ACP agent stopped");
	Ok(CommandOutcome::Ok)
}

command!(acp_toggle, {
	aliases: &["acp.toggle", "acp"],
	description: "Toggle the ACP chat panel"
}, handler: cmd_acp_toggle);

fn cmd_acp_toggle(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.editor.acp_toggle()?;
	Ok(CommandOutcome::Ok)
}

command!(acp_insert_last, {
	aliases: &["acp.insert_last"],
	description: "Insert the last ACP assistant response"
}, handler: cmd_acp_insert_last);

fn cmd_acp_insert_last(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.editor.acp_insert_last()?;
	Ok(CommandOutcome::Ok)
}

command!(acp_cancel, {
	aliases: &["acp.cancel"],
	description: "Cancel the current ACP request"
}, handler: cmd_acp_cancel);

fn cmd_acp_cancel(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.editor.acp_cancel()?;
	ctx.message("ACP request cancelled");
	Ok(CommandOutcome::Ok)
}
