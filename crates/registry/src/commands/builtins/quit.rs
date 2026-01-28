use crate::command;
use crate::commands::{CommandOutcome};

command!(quit, {
	aliases: &["q"],
	description: "Quit editor",
}, handler: |_ctx| {
	Box::pin(async move {
		Ok(CommandOutcome::Quit)
	})
});

command!(force_quit, {
	aliases: &["q!"],
	description: "Force quit editor",
}, handler: |_ctx| {
	Box::pin(async move {
		Ok(CommandOutcome::ForceQuit)
	})
});

pub const DEFS: &[&crate::commands::CommandDef] = &[
	&CMD_quit,
	&CMD_force_quit,
];
