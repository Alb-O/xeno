//! Theme command for switching editor color schemes.

use evildoer_manifest::{
	COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome, RegistrySource, flags,
};
use futures::future::LocalBoxFuture;
use linkme::distributed_slice;

#[allow(non_upper_case_globals)]
#[distributed_slice(COMMANDS)]
pub static CMD_theme: CommandDef = CommandDef {
	id: "evildoer-stdlib::theme",
	name: "theme",
	aliases: &["colorscheme"],
	description: "Set the editor theme",
	handler: cmd_theme,
	user_data: None,
	priority: 0,
	source: RegistrySource::Builtin,
	required_caps: &[],
	flags: flags::NONE,
};

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
