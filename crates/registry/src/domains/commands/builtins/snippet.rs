use xeno_primitives::BoxFutureLocal;

use crate::command_handler;
use crate::commands::{CommandContext, CommandError, CommandOutcome};

command_handler!(snippet, handler: cmd_snippet);

fn cmd_snippet<'a>(ctx: &'a mut CommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		if ctx.args.is_empty() {
			return Err(CommandError::MissingArgument("snippet body"));
		}

		let body = ctx.args.join(" ");
		if !ctx.editor.insert_snippet_body(&body) {
			return Err(CommandError::Failed("Failed to insert snippet body".to_string()));
		}

		Ok(CommandOutcome::Ok)
	})
}
