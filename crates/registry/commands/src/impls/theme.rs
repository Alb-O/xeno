//! Theme command for switching editor color schemes.

use futures::future::LocalBoxFuture;

use crate::{command, CommandContext, CommandError, CommandOutcome, RegistrySource};

command!(
	theme,
	{
		aliases: &["colorscheme"],
		description: "Set the editor theme",
		source: RegistrySource::Builtin,
	},
	handler: cmd_theme
);

fn cmd_theme<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let theme_name = ctx
			.args
			.first()
			.ok_or(CommandError::MissingArgument("theme name"))?;
		ctx.editor.set_theme(theme_name)?;
		ctx.editor
			.notify("info", &format!("Theme set to '{}'", theme_name));
		Ok(CommandOutcome::Ok)
	})
}
