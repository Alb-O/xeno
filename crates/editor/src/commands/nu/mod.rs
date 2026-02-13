//! Nu macro commands.

use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;
use xeno_registry::notifications::keys;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::types::{InvocationPolicy, InvocationResult};
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

		if ctx.editor.nu_runtime().is_none() {
			let Some(config_dir) = crate::paths::get_config_dir() else {
				return Err(CommandError::Failed("config directory is unavailable; cannot auto-load xeno.nu".to_string()));
			};
			reload_runtime_from_dir(ctx.editor, config_dir).await?;
		}

		let runtime = ctx
			.editor
			.nu_runtime()
			.cloned()
			.ok_or_else(|| CommandError::Failed("Nu runtime is not loaded".to_string()))?;

		let fn_name = (*fn_name).to_string();
		let args: Vec<String> = ctx.args.iter().skip(1).map(|arg| (*arg).to_string()).collect();
		let invocations = tokio::task::spawn_blocking(move || runtime.run_invocations(&fn_name, &args))
			.await
			.map_err(|error| CommandError::Failed(format!("failed to join nu-run task: {error}")))?
			.map_err(CommandError::Failed)?;

		if invocations.is_empty() {
			return Err(CommandError::Failed("nu-run produced no invocations".to_string()));
		}

		for invocation in invocations {
			let describe = invocation.describe();
			match ctx.editor.run_invocation(invocation, InvocationPolicy::enforcing()).await {
				InvocationResult::Ok => {}
				InvocationResult::Quit => return Ok(CommandOutcome::Quit),
				InvocationResult::ForceQuit => return Ok(CommandOutcome::ForceQuit),
				InvocationResult::NotFound(target) => {
					return Err(CommandError::Failed(format!("nu-run invocation not found: {target} ({describe})")));
				}
				InvocationResult::CapabilityDenied(cap) => {
					return Err(CommandError::Failed(format!("nu-run invocation denied by capability {cap:?} ({describe})")));
				}
				InvocationResult::ReadonlyDenied => {
					return Err(CommandError::Failed(format!("nu-run invocation blocked by readonly mode ({describe})")));
				}
				InvocationResult::CommandError(error) => {
					return Err(CommandError::Failed(format!("nu-run invocation failed: {error} ({describe})")));
				}
			}
		}

		Ok(CommandOutcome::Ok)
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
