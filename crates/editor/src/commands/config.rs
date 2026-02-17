//! Config reload commands.

use xeno_primitives::BoxFutureLocal;
use xeno_registry::notifications::keys;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;

editor_command!(
	reload_config,
	{
		keys: &["reload-config", "config-reload"],
		description: "Reload user config from disk"
	},
	handler: cmd_reload_config
);

fn cmd_reload_config<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let Some(config_dir) = crate::paths::get_config_dir() else {
			ctx.editor.notify(keys::warn("Config directory is unavailable"));
			return Ok(CommandOutcome::Ok);
		};

		let report = ctx
			.editor
			.state
			.worker_runtime
			.spawn_blocking(xeno_worker::TaskClass::IoBlocking, move || {
				xeno_registry::config::load::load_user_config_from_dir(&config_dir)
			})
			.await
			.map_err(|error| CommandError::Failed(format!("failed to join config reload task: {error}")))?;

		for (path, warning) in &report.warnings {
			tracing::warn!(path = %path.display(), "{warning}");
		}
		for (path, error) in &report.errors {
			tracing::warn!(path = %path.display(), error = %error, "failed to load config");
		}

		let can_apply = report.config.is_some() || report.errors.is_empty();
		if can_apply {
			ctx.editor.apply_loaded_config(report.config);
			ctx.editor.kick_theme_load();
		}

		if !can_apply {
			ctx.editor.notify(keys::warn("Config reload failed; keeping existing config"));
			return Ok(CommandOutcome::Ok);
		}

		if !report.errors.is_empty() {
			ctx.editor.notify(keys::warn(format!(
				"Config reloaded with {} error(s) and {} warning(s)",
				report.errors.len(),
				report.warnings.len()
			)));
		} else if !report.warnings.is_empty() {
			ctx.editor
				.notify(keys::warn(format!("Config reloaded with {} warning(s)", report.warnings.len())));
		} else {
			ctx.editor.notify(keys::success("Config reloaded"));
		}

		// Report keymap build problems from key overrides.
		let keymap = ctx.editor.effective_keymap();
		let problems = keymap.problems();
		if !problems.is_empty() {
			for p in problems.iter().take(5) {
				let mode_str = p.mode.map_or("?".to_string(), |m| format!("{m:?}"));
				ctx.editor
					.notify(keys::warn(format!("keys: [{mode_str}] {} → {}: {}", p.keys, p.target, p.message)));
			}
			if problems.len() > 5 {
				ctx.editor.notify(keys::warn(format!("... and {} more keymap problem(s)", problems.len() - 5)));
			}
		}

		Ok(CommandOutcome::Ok)
	})
}
