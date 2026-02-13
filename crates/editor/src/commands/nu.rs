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
mod tests {
	use super::*;
	use crate::types::Invocation;

	fn write_script(dir: &std::path::Path, source: &str) {
		std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
	}

	#[test]
	fn parse_invocation_variants() {
		assert!(matches!(
			crate::nu::parse_invocation_spec("action:move_right").expect("action should parse"),
			Invocation::Action { .. }
		));
		assert!(matches!(
			crate::nu::parse_invocation_spec("command:help themes").expect("command should parse"),
			Invocation::Command { .. }
		));
		assert!(matches!(
			crate::nu::parse_invocation_spec("editor:stats").expect("editor command should parse"),
			Invocation::EditorCommand { .. }
		));
	}

	#[test]
	fn nu_run_dispatches_action() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def go [name] { $\"action:($name)\" }");

		let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
		let mut editor = Editor::new_scratch();
		editor.set_nu_runtime(Some(runtime));

		let action_name = if xeno_registry::find_action("move_right").is_some() {
			"move_right".to_string()
		} else {
			xeno_registry::all_actions()
				.first()
				.map(|action| action.name_str().to_string())
				.expect("registry should include at least one action")
		};

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let result = rt.block_on(editor.run_invocation(
			Invocation::editor_command("nu-run", vec!["go".to_string(), action_name]),
			InvocationPolicy::enforcing(),
		));

		assert!(matches!(result, InvocationResult::Ok));
	}

	#[test]
	fn nu_run_dispatches_editor_command() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def go [] { \"editor:stats\" }");

		let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
		let mut editor = Editor::new_scratch();
		editor.set_nu_runtime(Some(runtime));

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let result = rt.block_on(editor.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing()));

		assert!(matches!(result, InvocationResult::Ok));
	}

	#[test]
	fn nu_reload_rejects_external_script_and_keeps_existing_runtime() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def ok [] { \"editor:stats\" }");

		let mut editor = Editor::new_scratch();
		let initial_runtime = crate::nu::NuRuntime::load(temp.path()).expect("initial runtime should load");
		let initial_script = initial_runtime.script_path().to_path_buf();
		editor.set_nu_runtime(Some(initial_runtime));

		write_script(temp.path(), "^echo hi");

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let err = rt
			.block_on(reload_runtime_from_dir(&mut editor, temp.path().to_path_buf()))
			.expect_err("external scripts should be rejected");

		assert!(matches!(err, CommandError::Failed(_)));
		let kept_runtime = editor.nu_runtime().expect("existing runtime should be kept");
		assert_eq!(kept_runtime.script_path(), initial_script);
	}

	#[test]
	fn action_post_hook_dispatches_once_with_recursion_guard() {
		assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(
			temp.path(),
			"export def on_action_post [name result] { if $name == \"move_right\" and $result == \"ok\" { \"action:move_right\" } else { [] } }",
		);

		let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
		let mut editor = Editor::from_content("abcd".to_string(), None);
		editor.set_nu_runtime(Some(runtime));

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let result = rt.block_on(editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()));

		assert!(matches!(result, InvocationResult::Ok));
		assert_eq!(editor.buffer().cursor, 2, "hook should add exactly one extra move_right invocation");
	}

	#[test]
	fn action_post_missing_hook_is_noop() {
		assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def unrelated [] { [] }");

		let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
		let mut editor = Editor::from_content("abcd".to_string(), None);
		editor.set_nu_runtime(Some(runtime));

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let result = rt.block_on(editor.run_invocation(Invocation::action("move_right"), InvocationPolicy::enforcing()));

		assert!(matches!(result, InvocationResult::Ok));
		assert_eq!(editor.buffer().cursor, 1, "without on_action_post hook only base action should run");
	}

	#[test]
	fn nu_run_structured_action_record_executes_count() {
		assert!(xeno_registry::find_action("move_right").is_some(), "expected move_right action to exist");

		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def go [] { { kind: \"action\", name: \"move_right\", count: 2 } }");

		let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
		let mut editor = Editor::from_content("abcd".to_string(), None);
		editor.set_nu_runtime(Some(runtime));

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let result = rt.block_on(editor.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing()));

		assert!(matches!(result, InvocationResult::Ok));
		assert_eq!(editor.buffer().cursor, 2, "structured action record should honor count");
	}

	#[test]
	fn nu_run_structured_list_of_records_executes() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(
			temp.path(),
			"export def go [] { [\n  { kind: \"editor\", name: \"stats\" },\n  { kind: \"command\", name: \"help\" }\n] }",
		);

		let runtime = crate::nu::NuRuntime::load(temp.path()).expect("runtime should load");
		let mut editor = Editor::new_scratch();
		editor.set_nu_runtime(Some(runtime));

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("runtime should build");
		let result = rt.block_on(editor.run_invocation(Invocation::editor_command("nu-run", vec!["go".to_string()]), InvocationPolicy::enforcing()));

		assert!(matches!(result, InvocationResult::Ok));
	}
}
