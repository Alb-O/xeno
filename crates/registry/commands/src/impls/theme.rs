//! Theme command for switching editor color schemes.

use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, RegistrySource, command};

command!(
	theme,
	{
		aliases: &["colorscheme"],
		description: "Set the editor theme",
		source: RegistrySource::Builtin,
	},
	handler: cmd_theme
);

/// Handler for the `:theme` command.
fn cmd_theme<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let theme_name = ctx
			.args
			.first()
			.ok_or(CommandError::MissingArgument("theme name"))?;
		ctx.editor.set_theme(theme_name)?;
		ctx.emit(keys::theme_set::call(theme_name));
		Ok(CommandOutcome::Ok)
	})
}
