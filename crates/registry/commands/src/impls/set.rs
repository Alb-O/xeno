//! Set option commands.

use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, RegistrySource, command};

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

/// Handler for the `:set` command.
///
/// Accepts either `option=value` or `option value` syntax.
/// For boolean options, `option` alone sets to true and `nooption` sets to false.
fn cmd_set<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			// TODO: Show all options that differ from defaults
			return Ok(CommandOutcome::Ok);
		}

		let (key, value) = parse_set_args(ctx.args)?;
		ctx.editor.set_option(&key, &value)?;
		ctx.emit(keys::option_set::call(&key, &value));
		Ok(CommandOutcome::Ok)
	})
}

/// Handler for the `:setlocal` command.
///
/// Same syntax as `:set`, but applies the option only to the current buffer.
fn cmd_setlocal<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			// TODO: Show all buffer-local options
			return Ok(CommandOutcome::Ok);
		}

		let (key, value) = parse_set_args(ctx.args)?;
		ctx.editor.set_local_option(&key, &value)?;
		ctx.emit(keys::option_set::call(&key, &value));
		Ok(CommandOutcome::Ok)
	})
}

/// Parses `:set` arguments into (key, value).
///
/// Supports multiple formats:
/// - `option=value` (e.g., `tab-width=4`)
/// - `option value` (e.g., `tab-width 4`)
/// - `option` for boolean true (e.g., `cursorline`)
/// - `nooption` for boolean false (e.g., `nocursorline`)
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
