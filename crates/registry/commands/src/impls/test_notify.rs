use futures::future::LocalBoxFuture;

use crate::{command, CommandContext, CommandError, CommandOutcome};

command!(
	test_notify,
	{ aliases: &[], description: "Test the new notification system" },
	handler: test_notify
);

pub fn test_notify<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor
			.notify("warn", "This is a test notification via distributed slices!");
		Ok(CommandOutcome::Ok)
	})
}
