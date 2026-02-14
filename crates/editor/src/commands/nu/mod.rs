//! Nu macro commands.

use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;
use xeno_registry::notifications::keys;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::types::{Invocation, InvocationPolicy, InvocationResult};
use crate::{Editor, editor_command};

editor_command!(
	nu_reload,
	{
		keys: &["nu-reload", "reload-nu"],
		description: "Reload ~/.config/xeno/xeno.nu"
	},
	handler: cmd_nu_reload
);

editor_command!(
	nu_run,
	{
		keys: &["nu-run"],
		description: "Run a Nu macro function"
	},
	handler: cmd_nu_run
);

fn cmd_nu_reload<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let Some(config_dir) = crate::paths::get_config_dir() else {
			ctx.editor.notify(keys::warn("Config directory is unavailable"));
			return Ok(CommandOutcome::Ok);
		};

		match reload_runtime_from_dir(ctx.editor, config_dir).await {
			Ok(script_path) => {
				ctx.editor.notify(keys::success(format!("Loaded Nu macros from {}", script_path.display())));
			}
			Err(error) => {
				ctx.editor
					.notify(keys::warn(format!("Failed to reload Nu macros: {error} (keeping existing runtime)")));
			}
		}
		Ok(CommandOutcome::Ok)
	})
}

fn cmd_nu_run<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let Some(fn_name) = ctx.args.first() else {
			return Err(CommandError::MissingArgument("fn"));
		};

		let invocation = Invocation::Nu {
			name: (*fn_name).to_string(),
			args: ctx.args.iter().skip(1).map(|arg| (*arg).to_string()).collect(),
		};
		let describe = invocation.describe();

		match ctx.editor.run_invocation(invocation, InvocationPolicy::enforcing()).await {
			InvocationResult::Ok => Ok(CommandOutcome::Ok),
			InvocationResult::Quit => Ok(CommandOutcome::Quit),
			InvocationResult::ForceQuit => Ok(CommandOutcome::ForceQuit),
			InvocationResult::NotFound(target) => Err(CommandError::Failed(format!("nu-run invocation not found: {target} ({describe})"))),
			InvocationResult::CapabilityDenied(cap) => Err(CommandError::Failed(format!("nu-run invocation denied by capability {cap:?} ({describe})"))),
			InvocationResult::ReadonlyDenied => Err(CommandError::Failed(format!("nu-run invocation blocked by readonly mode ({describe})"))),
			InvocationResult::CommandError(error) => Err(CommandError::Failed(format!("nu-run invocation failed: {error} ({describe})"))),
		}
	})
}

async fn reload_runtime_from_dir(editor: &mut Editor, config_dir: PathBuf) -> Result<PathBuf, CommandError> {
	let loaded = tokio::task::spawn_blocking(move || crate::nu::NuRuntime::load(&config_dir))
		.await
		.map_err(|error| CommandError::Failed(format!("failed to join Nu runtime load task: {error}")))?;

	match loaded {
		Ok(runtime) => {
			let script_path = runtime.script_path().to_path_buf();
			editor.set_nu_runtime(Some(runtime));
			Ok(script_path)
		}
		Err(error) => Err(CommandError::Failed(error)),
	}
}

#[cfg(test)]
mod tests;
