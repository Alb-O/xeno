use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome};
use crate::notifications::keys;

command_handler!(theme, handler: cmd_theme);

fn cmd_theme<'a>(ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let theme_name = ctx.args.first().ok_or(CommandError::MissingArgument("theme name"))?;
		ctx.editor.set_theme(theme_name)?;
		ctx.emit(keys::theme_set(theme_name));
		Ok(CommandOutcome::Ok)
	})
}
