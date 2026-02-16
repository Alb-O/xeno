//! Nu runtime for editor macro scripts.

pub(crate) mod coordinator;
pub(crate) mod ctx;
pub(crate) mod executor;
pub(crate) mod pipeline;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub use xeno_invocation::nu::{DecodeLimits, decode_runtime_invocations_with_limits};
use xeno_nu_runtime::{FunctionId, Runtime as ScriptRuntime};
use xeno_nu_value::Value;

use crate::types::Invocation;

/// Cached function IDs for hook functions, populated once when the runtime is set.
#[derive(Clone, Debug, Default)]
pub(crate) struct CachedHookIds {
	pub on_action_post: Option<FunctionId>,
	pub on_command_post: Option<FunctionId>,
	pub on_editor_command_post: Option<FunctionId>,
	pub on_mode_change: Option<FunctionId>,
	pub on_buffer_open: Option<FunctionId>,
}

/// Hook function identifiers used to select a cached function ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

const SLOW_CALL_THRESHOLD: Duration = Duration::from_millis(5);

/// Loaded Nu macro script runtime state.
#[derive(Clone)]
pub struct NuRuntime {
	config_dir: PathBuf,
	script_path: PathBuf,
	runtime: ScriptRuntime,
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
		let runtime = ScriptRuntime::load(config_dir)?;
		let script_path = runtime.script_path().to_path_buf();
		Ok(Self {
			config_dir: config_dir.to_path_buf(),
			script_path,
			runtime,
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
		decode_runtime_invocations_with_limits(value, limits)
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
			Ok(value) => decode_runtime_invocations_with_limits(value, limits).map(Some),
			Err(NuRunError::MissingFunction(_)) => Ok(None),
			Err(NuRunError::Other(error)) => Err(error),
		}
	}

	/// Look up a script-defined declaration by name. Returns `None` for
	/// missing functions and builtins.
	pub fn find_script_decl(&self, name: &str) -> Option<FunctionId> {
		self.runtime.resolve_function(name)
	}

	/// Run a pre-resolved declaration and decode into invocations.
	pub fn run_invocations_by_decl_id(
		&self,
		decl_id: FunctionId,
		args: &[String],
		limits: DecodeLimits,
		env: &[(&str, Value)],
	) -> Result<Vec<Invocation>, String> {
		let value = self.call_by_decl_id(decl_id, args, env)?;
		decode_runtime_invocations_with_limits(value, limits)
	}

	/// Run a pre-resolved declaration with owned args/env (zero-clone hot path).
	pub fn run_invocations_by_decl_id_owned(
		&self,
		decl_id: FunctionId,
		args: Vec<String>,
		limits: DecodeLimits,
		env: Vec<(String, Value)>,
	) -> Result<Vec<Invocation>, String> {
		let start = Instant::now();
		let value = self.runtime.call_owned(decl_id, args, env)?;
		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}
		decode_runtime_invocations_with_limits(value, limits)
	}

	fn call_by_decl_id(&self, decl_id: FunctionId, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
		let start = Instant::now();
		let value = self.runtime.call(decl_id, args, env)?;
		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}
		Ok(value)
	}

	fn run_internal(&self, fn_name: &str, args: &[String], env: &[(&str, Value)]) -> Result<Value, NuRunError> {
		let start = Instant::now();

		let decl_id = self
			.runtime
			.resolve_function(fn_name)
			.ok_or_else(|| NuRunError::MissingFunction(fn_name.to_string()))?;

		let value = self.runtime.call(decl_id, args, env).map_err(NuRunError::Other)?;

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
mod tests;
