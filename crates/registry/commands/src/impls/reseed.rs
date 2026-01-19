//! Reseed command for re-extracting embedded runtime assets.

use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, RegistrySource, command};

command!(
	reseed,
	{
		description: "Re-extract embedded themes and queries to runtime directory",
		source: RegistrySource::Builtin,
	},
	handler: cmd_reseed
);

/// Handler for the `:reseed` command.
fn cmd_reseed<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		xeno_runtime_language::reseed_runtime()
			.map_err(|e| CommandError::Other(format!("Failed to reseed runtime: {e}")))?;

		ctx.emit(keys::success("Runtime assets reseeded successfully"));
		Ok(CommandOutcome::Ok)
	})
}
