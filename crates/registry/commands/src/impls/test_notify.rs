use futures::future::LocalBoxFuture;

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
		ctx.editor.notify(
			"warn",
			"This is a test notification via distributed slices!",
		);
		Ok(CommandOutcome::Ok)
	})
}
