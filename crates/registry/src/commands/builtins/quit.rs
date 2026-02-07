use crate::command_handler;
use crate::commands::CommandOutcome;

command_handler!(quit, handler: |_ctx| {
	Box::pin(async move {
		Ok(CommandOutcome::Quit)
	})
});

command_handler!(force_quit, handler: |_ctx| {
	Box::pin(async move {
		Ok(CommandOutcome::ForceQuit)
	})
});
