use futures::future::LocalBoxFuture;

use crate::command;
use crate::commands::{CommandContext, CommandError, CommandOutcome, RegistrySource};
use crate::notifications::keys;

command!(
	set,
	{
		aliases: &["se"],
		description: "Set an option globally",
		source: RegistrySource::Builtin,
	},
	handler: cmd_set
);

command!(
	setlocal,
	{
		aliases: &["setl"],
		description: "Set an option for current buffer only",
		source: RegistrySource::Builtin,
	},
	handler: cmd_setlocal
);

fn cmd_set<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Ok(CommandOutcome::Ok);
		}

		let (key, value) = parse_set_args(ctx.args)?;
		ctx.editor.set_option(&key, &value)?;
		ctx.emit(keys::option_set(&key, &value));
		Ok(CommandOutcome::Ok)
	})
}

fn cmd_setlocal<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Ok(CommandOutcome::Ok);
		}

		let (key, value) = parse_set_args(ctx.args)?;
		ctx.editor.set_local_option(&key, &value)?;
		ctx.emit(keys::option_set(&key, &value));
		Ok(CommandOutcome::Ok)
	})
}

fn parse_set_args(args: &[&str]) -> Result<(String, String), CommandError> {
	let first = args[0];

	if let Some((key, value)) = first.split_once('=') {
		return Ok((key.to_string(), value.to_string()));
	}

	if args.len() >= 2 {
		return Ok((first.to_string(), args[1..].join(" ")));
	}

	if let Some(opt) = first.strip_prefix("no") {
		Ok((opt.to_string(), "false".to_string()))
	} else {
		Ok((first.to_string(), "true".to_string()))
	}
}

pub const DEFS: &[&crate::commands::CommandDef] = &[
	&CMD_set,
	&CMD_setlocal,
];
