use linkme::distributed_slice;

use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

#[distributed_slice(COMMANDS)]
static CMD_PERMIT: CommandDef = CommandDef {
	name: "permit",
	aliases: &[],
	description: "Respond to a plugin permission request (:permit <id> <option>)",
	handler: cmd_permit,
};

fn cmd_permit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
	let Some(&id_str) = ctx.args.first() else {
		return Err(CommandError::MissingArgument("id"));
	};
	let Some(&option) = ctx.args.get(1) else {
		return Err(CommandError::MissingArgument("option"));
	};

	let id: u64 = id_str
		.parse()
		.map_err(|_| CommandError::InvalidArgument("id must be a number".into()))?;

	ctx.editor
		.on_permission_decision(id, option)
		.map_err(CommandError::Failed)?;

	Ok(CommandOutcome::Ok)
}
