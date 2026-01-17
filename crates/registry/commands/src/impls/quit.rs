use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(quit, { aliases: &["q"], description: "Quit the editor" }, handler: cmd_quit);

/// Handler for the `:quit` command.
fn cmd_quit<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.editor.is_modified() {
			ctx.emit(keys::UNSAVED_CHANGES_FORCE_QUIT);
			return Ok(CommandOutcome::Ok);
		}
		Ok(CommandOutcome::Quit)
	})
}

command!(
	quit_force,
	{ aliases: &["q!"], description: "Quit without saving" },
	handler: cmd_quit_force
);

/// Handler for the `:quit-force` command.
fn cmd_quit_force<'a>(
	_ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move { Ok(CommandOutcome::ForceQuit) })
}
