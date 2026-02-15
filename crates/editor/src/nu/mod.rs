//! Nu runtime for editor macro scripts.

pub(crate) mod ctx;
mod decode;
pub(crate) mod executor;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use decode::{DecodeLimits, decode_invocations, decode_invocations_with_limits};
use nu_protocol::engine::EngineState;
use nu_protocol::{DeclId, Value};

use crate::types::Invocation;

/// Cached decl IDs for hook functions, populated once when the runtime is set.
#[derive(Clone, Debug, Default)]
pub(crate) struct CachedHookIds {
	pub on_action_post: Option<DeclId>,
	pub on_command_post: Option<DeclId>,
	pub on_editor_command_post: Option<DeclId>,
	pub on_mode_change: Option<DeclId>,
	pub on_buffer_open: Option<DeclId>,
}

/// Hook function identifiers used to select a cached decl ID.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NuHook {
	ActionPost,
	CommandPost,
	EditorCommandPost,
	ModeChange,
	BufferOpen,
}

impl NuHook {
	pub const fn fn_name(self) -> &'static str {
		match self {
			Self::ActionPost => "on_action_post",
			Self::CommandPost => "on_command_post",
			Self::EditorCommandPost => "on_editor_command_post",
			Self::ModeChange => "on_mode_change",
			Self::BufferOpen => "on_buffer_open",
		}
	}
}

const SCRIPT_FILE_NAME: &str = "xeno.nu";
const SLOW_CALL_THRESHOLD: Duration = Duration::from_millis(5);

/// Loaded Nu macro script runtime state.
#[derive(Clone)]
pub struct NuRuntime {
	config_dir: PathBuf,
	script_path: PathBuf,
	base_engine: Arc<EngineState>,
	/// Declarations introduced by `xeno.nu` (and its module imports).
	/// Only these may be called externally; builtins are denied.
	script_decls: Arc<HashSet<DeclId>>,
}

impl std::fmt::Debug for NuRuntime {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NuRuntime")
			.field("config_dir", &self.config_dir)
			.field("script_path", &self.script_path)
			.finish_non_exhaustive()
	}
}

impl NuRuntime {
	/// Load and validate the `xeno.nu` script from the given config directory.
	pub fn load(config_dir: &Path) -> Result<Self, String> {
		let script_path = config_dir.join(SCRIPT_FILE_NAME);
		let script_src = std::fs::read_to_string(&script_path).map_err(|error| format!("failed to read {}: {error}", script_path.display()))?;

		let (base_engine, script_decls) = build_base_engine(config_dir, &script_path, &script_src)?;

		Ok(Self {
			config_dir: config_dir.to_path_buf(),
			script_path,
			base_engine: Arc::new(base_engine),
			script_decls: Arc::new(script_decls),
		})
	}

	/// Returns the loaded script path.
	pub fn script_path(&self) -> &Path {
		&self.script_path
	}

	/// Run a function in `xeno.nu` and return its raw Nu value.
	pub fn run(&self, fn_name: &str, args: &[String]) -> Result<Value, String> {
		self.run_internal(fn_name, args, &[]).map_err(map_run_error)
	}

	/// Run a function and decode its return value into structured invocations.
	pub fn run_invocations(&self, fn_name: &str, args: &[String]) -> Result<Vec<Invocation>, String> {
		self.run_invocations_with_limits(fn_name, args, DecodeLimits::macro_defaults())
	}

	/// Run a function and decode its return value into structured invocations with explicit decode limits.
	pub fn run_invocations_with_limits(&self, fn_name: &str, args: &[String], limits: DecodeLimits) -> Result<Vec<Invocation>, String> {
		self.run_invocations_with_limits_and_env(fn_name, args, limits, &[])
	}

	/// Run a function and decode its return value into structured invocations with explicit decode limits and env vars.
	pub fn run_invocations_with_limits_and_env(
		&self,
		fn_name: &str,
		args: &[String],
		limits: DecodeLimits,
		env: &[(&str, Value)],
	) -> Result<Vec<Invocation>, String> {
		let value = self.run_internal(fn_name, args, env).map_err(map_run_error)?;
		decode_invocations_with_limits(value, limits)
	}

	/// Run a function and decode structured invocations, returning `None` when the function is absent.
	pub fn try_run_invocations(&self, fn_name: &str, args: &[String]) -> Result<Option<Vec<Invocation>>, String> {
		self.try_run_invocations_with_limits(fn_name, args, DecodeLimits::macro_defaults())
	}

	/// Run a function and decode structured invocations with explicit limits, returning `None` when the function is absent.
	pub fn try_run_invocations_with_limits(&self, fn_name: &str, args: &[String], limits: DecodeLimits) -> Result<Option<Vec<Invocation>>, String> {
		self.try_run_invocations_with_limits_and_env(fn_name, args, limits, &[])
	}

	/// Run a function and decode structured invocations with explicit limits and env vars, returning `None` when the function is absent.
	pub fn try_run_invocations_with_limits_and_env(
		&self,
		fn_name: &str,
		args: &[String],
		limits: DecodeLimits,
		env: &[(&str, Value)],
	) -> Result<Option<Vec<Invocation>>, String> {
		match self.run_internal(fn_name, args, env) {
			Ok(value) => decode_invocations_with_limits(value, limits).map(Some),
			Err(NuRunError::MissingFunction(_)) => Ok(None),
			Err(NuRunError::Other(error)) => Err(error),
		}
	}

	/// Look up a script-defined declaration by name. Returns `None` for
	/// missing functions and builtins.
	pub fn find_script_decl(&self, name: &str) -> Option<DeclId> {
		let decl_id = xeno_nu::find_decl(&self.base_engine, name)?;
		self.script_decls.contains(&decl_id).then_some(decl_id)
	}

	/// Run a pre-resolved declaration and decode into invocations.
	pub fn run_invocations_by_decl_id(&self, decl_id: DeclId, args: &[String], limits: DecodeLimits, env: &[(&str, Value)]) -> Result<Vec<Invocation>, String> {
		let value = self.call_by_decl_id(decl_id, args, env)?;
		decode_invocations_with_limits(value, limits)
	}

	/// Run a pre-resolved declaration with owned args/env (zero-clone hot path).
	pub fn run_invocations_by_decl_id_owned(
		&self,
		decl_id: DeclId,
		args: Vec<String>,
		limits: DecodeLimits,
		env: Vec<(String, Value)>,
	) -> Result<Vec<Invocation>, String> {
		let start = Instant::now();
		let value = xeno_nu::call_function_owned(&self.base_engine, decl_id, args, env)?;
		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}
		decode_invocations_with_limits(value, limits)
	}

	fn call_by_decl_id(&self, decl_id: DeclId, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
		let start = Instant::now();
		let value = xeno_nu::call_function(&self.base_engine, decl_id, args, env)?;
		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}
		Ok(value)
	}

	fn run_internal(&self, fn_name: &str, args: &[String], env: &[(&str, Value)]) -> Result<Value, NuRunError> {
		let start = Instant::now();

		let decl_id = xeno_nu::find_decl(&self.base_engine, fn_name).ok_or_else(|| NuRunError::MissingFunction(fn_name.to_string()))?;

		if !self.script_decls.contains(&decl_id) {
			return Err(NuRunError::MissingFunction(fn_name.to_string()));
		}

		let value = xeno_nu::call_function(&self.base_engine, decl_id, args, env).map_err(NuRunError::Other)?;

		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(function = fn_name, elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}

		Ok(value)
	}
}

#[derive(Debug)]
enum NuRunError {
	MissingFunction(String),
	Other(String),
}

fn map_run_error(error: NuRunError) -> String {
	match error {
		NuRunError::MissingFunction(name) => {
			format!("Nu runtime error: function '{name}' is not defined in xeno.nu")
		}
		NuRunError::Other(msg) => msg,
	}
}

fn build_base_engine(config_dir: &Path, script_path: &Path, script_src: &str) -> Result<(EngineState, HashSet<DeclId>), String> {
	let mut engine_state = xeno_nu::create_engine_state(Some(config_dir))?;
	let fname = script_path.to_string_lossy().to_string();
	let parsed = xeno_nu::parse_and_validate_with_policy(&mut engine_state, &fname, script_src, Some(config_dir), xeno_nu::ParsePolicy::ModuleOnly)?;
	Ok((engine_state, parsed.script_decl_ids.into_iter().collect()))
}

/// Parse a macro invocation spec string into an [`Invocation`].
pub fn parse_invocation_spec(spec: &str) -> Result<Invocation, String> {
	let parsed = xeno_invocation_spec::parse_spec(spec)?;
	match parsed.kind {
		xeno_invocation_spec::SpecKind::Action => Ok(Invocation::action(parsed.name)),
		xeno_invocation_spec::SpecKind::Command => Ok(Invocation::command(parsed.name, parsed.args)),
		xeno_invocation_spec::SpecKind::Editor => Ok(Invocation::editor_command(parsed.name, parsed.args)),
		xeno_invocation_spec::SpecKind::Nu => Ok(Invocation::nu(parsed.name, parsed.args)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn write_script(dir: &Path, source: &str) {
		std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
	}

	#[test]
	fn load_rejects_external_calls() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "^echo hi");
		let err = NuRuntime::load(temp.path()).expect_err("external calls should be rejected");
		let err_lower = err.to_lowercase();
		assert!(err_lower.contains("external") || err_lower.contains("parse error"), "{err}");
	}

	#[test]
	fn run_invocations_supports_record_and_list() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(
			temp.path(),
			"export def one [] { editor stats }\nexport def many [] { [(editor stats), (command help)] }",
		);

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

		let one = runtime.run_invocations("one", &[]).expect("record return should decode");
		assert!(matches!(one.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));

		let many = runtime.run_invocations("many", &[]).expect("list return should decode");
		assert_eq!(many.len(), 2);
	}

	#[test]
	fn run_invocations_supports_alias_entrypoint() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export alias go = editor stats");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let invocations = runtime.run_invocations("go", &[]).expect("alias entrypoint should run");
		assert!(matches!(invocations.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
	}

	#[test]
	fn run_invocations_supports_structured_records() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(
			temp.path(),
			"export def action_rec [] { { kind: \"action\", name: \"move_right\", count: 2, extend: true, register: \"a\" } }\n\
export def action_char [] { { kind: \"action\", name: \"find_char\", char: \"x\" } }\n\
export def mixed [] { [ { kind: \"editor\", name: \"stats\" }, { kind: \"command\", name: \"help\", args: [\"themes\"] } ] }\n\
export def nested_nu [] { { kind: \"nu\", name: \"go\", args: [\"a\", \"b\"] } }",
		);

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

		let action = runtime.run_invocations("action_rec", &[]).expect("structured action should decode");
		assert!(matches!(
			action.as_slice(),
			[Invocation::Action {
				name,
				count: 2,
				extend: true,
				register: Some('a')
			}] if name == "move_right"
		));

		let action_char = runtime.run_invocations("action_char", &[]).expect("structured action-with-char should decode");
		assert!(matches!(
			action_char.as_slice(),
			[Invocation::ActionWithChar {
				name,
				char_arg: 'x',
				..
			}] if name == "find_char"
		));

		let mixed = runtime.run_invocations("mixed", &[]).expect("structured list should decode");
		assert!(matches!(mixed.first(), Some(Invocation::EditorCommand { name, .. }) if name == "stats"));
		assert!(matches!(mixed.get(1), Some(Invocation::Command { name, .. }) if name == "help"));

		let nested_nu = runtime.run_invocations("nested_nu", &[]).expect("structured nu invocation should decode");
		assert!(matches!(nested_nu.as_slice(), [Invocation::Nu { name, args }] if name == "go" && args == &["a".to_string(), "b".to_string()]));
	}

	#[test]
	fn decode_limits_cap_invocation_count() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def many [] { [(editor stats), (editor stats)] }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let err = runtime
			.run_invocations_with_limits(
				"many",
				&[],
				DecodeLimits {
					max_invocations: 1,
					..DecodeLimits::macro_defaults()
				},
			)
			.expect_err("decode limits should reject too many invocations");

		assert!(err.contains("invocation count"), "{err}");
	}

	#[test]
	fn run_allows_use_within_config_root() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		std::fs::write(temp.path().join("mod.nu"), "export def mk [] { editor stats }").expect("module should be writable");
		write_script(temp.path(), "use mod.nu *\nexport def go [] { mk }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let invocations = runtime.run_invocations("go", &[]).expect("run should succeed");
		assert!(matches!(invocations.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
	}

	#[test]
	fn try_run_returns_none_for_missing_function() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def known [] { editor stats }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let missing = runtime.try_run_invocations("missing", &[]).expect("missing function should be non-fatal");
		assert!(missing.is_none());
	}

	#[test]
	fn find_script_decl_rejects_builtins() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def go [] { editor stats }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		assert!(runtime.find_script_decl("go").is_some());
		assert!(runtime.find_script_decl("if").is_none());
		assert!(runtime.find_script_decl("nonexistent").is_none());
	}

	#[test]
	fn run_rejects_builtin_decls() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def go [] { editor stats }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

		let err = runtime.run("if", &[]).expect_err("builtin 'if' should be rejected");
		assert!(err.contains("not defined"), "expected 'not defined' error, got: {err}");

		let err = runtime.run("for", &[]).expect_err("builtin 'for' should be rejected");
		assert!(err.contains("not defined"), "expected 'not defined' error, got: {err}");

		let _ = runtime.run("go", &[]).expect("script function should succeed");
	}

	#[test]
	fn nothing_return_decodes_to_empty_invocations() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def noop [] { null }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let result = runtime.run_invocations("noop", &[]).expect("nothing return should decode");
		assert!(result.is_empty());
	}

	#[test]
	fn load_rejects_top_level_statement() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "42");
		let err = NuRuntime::load(temp.path()).expect_err("top-level expression should be rejected");
		assert!(err.contains("top-level") || err.contains("module-only"), "{err}");
	}

	#[test]
	fn load_rejects_export_extern_top_level() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export extern git []");
		let err = NuRuntime::load(temp.path()).expect_err("export extern should be rejected");
		assert!(
			err.contains("not allowed") || err.contains("extern") || err.contains("parse error") || err.contains("Unknown"),
			"{err}"
		);
	}

	#[test]
	fn load_allows_const_used_by_macro() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "const CMD = \"stats\"\nexport def go [] { editor $CMD }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let invocations = runtime.run_invocations("go", &[]).expect("run should succeed");
		assert!(matches!(invocations.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
	}
}
