//! Config reload commands.

use std::collections::HashMap;

use xeno_primitives::BoxFutureLocal;
use xeno_registry::notifications::keys;
use xeno_registry::options::OptionStore;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;
use crate::impls::Editor;

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

		let report = tokio::task::spawn_blocking(move || xeno_registry::config::load::load_user_config_from_dir(&config_dir))
			.await
			.map_err(|error| CommandError::Failed(format!("failed to join config reload task: {error}")))?;

		for (path, warning) in &report.warnings {
			tracing::warn!(path = %path.display(), "{warning}");
		}
		for (path, error) in &report.errors {
			tracing::warn!(path = %path.display(), error = %error, "failed to parse config");
		}

		apply_loaded_config(ctx.editor, report.config);
		ctx.editor.kick_theme_load();

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

		Ok(CommandOutcome::Ok)
	})
}

fn apply_loaded_config(editor: &mut Editor, config: Option<xeno_registry::config::Config>) {
	let mut key_overrides = None;
	let mut global_options = OptionStore::new();
	let mut language_options = HashMap::<String, OptionStore>::new();

	if let Some(mut config) = config {
		key_overrides = config.keys.take();
		global_options = config.options;

		for lang_config in config.languages {
			language_options.entry(lang_config.name).or_default().merge(&lang_config.options);
		}
	}

	editor.set_key_overrides(key_overrides);
	let editor_config = editor.config_mut();
	editor_config.global_options = global_options;
	editor_config.language_options = language_options;
}
