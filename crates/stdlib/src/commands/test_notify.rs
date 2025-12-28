use evildoer_manifest::{CommandContext, CommandError, CommandOutcome};
#[cfg(feature = "host")]
use futures::future::LocalBoxFuture;

#[cfg(feature = "host")]
use crate::command;

#[cfg(feature = "host")]
command!(test_notify, { aliases: &[], description: "Test the new notification system" }, handler: test_notify);

#[cfg(feature = "host")]
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
