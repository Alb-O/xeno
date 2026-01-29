use xeno_primitives::BoxFutureLocal;

use crate::command;
use crate::commands::{CommandContext, CommandError, CommandOutcome, RegistrySource};
use crate::notifications::keys;

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
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let theme_name = ctx
			.args
			.first()
			.ok_or(CommandError::MissingArgument("theme name"))?;
		ctx.editor.set_theme(theme_name)?;
		ctx.emit(keys::theme_set(theme_name));
		Ok(CommandOutcome::Ok)
	})
}

pub const DEFS: &[&crate::commands::CommandDef] = &[&CMD_theme];
