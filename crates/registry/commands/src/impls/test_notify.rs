use futures::future::LocalBoxFuture;
use xeno_registry_notifications::keys;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(
	test_notify,
	{ aliases: &[], description: "Test the new notification system" },
	handler: test_notify
);

/// Handler for the `:test-notify` command.
pub fn test_notify<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.emit(keys::warn::call(
			"This is a test notification via typed keys!",
		));
		Ok(CommandOutcome::Ok)
	})
}
