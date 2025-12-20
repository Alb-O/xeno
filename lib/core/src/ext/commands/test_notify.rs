#[cfg(feature = "host")]
use crate::command;
use crate::ext::{CommandContext, CommandError, CommandOutcome};

#[cfg(feature = "host")]
command!(test_notify, &[], "Test the new notification system", handler: test_notify);

pub fn test_notify(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	ctx.editor.notify(
		"warn",
		"This is a test notification via distributed slices!",
	);
	Ok(CommandOutcome::Ok)
}
