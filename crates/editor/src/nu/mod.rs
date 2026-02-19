//! Nu runtime for editor macro scripts.

pub(crate) mod coordinator;
pub(crate) mod ctx;
pub(crate) mod effects;
pub(crate) mod executor;
pub(crate) mod host;
pub(crate) mod pipeline;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub use xeno_invocation::nu::{DecodeBudget, NuCapability, NuEffect, NuEffectBatch, NuNotifyLevel, required_capability_for_effect};
use xeno_nu_api::{ExportId, NuProgram};
use xeno_nu_data::Value;

use crate::types::Invocation;

/// Cached function ID for the unified `on_hook` export, populated once when the runtime is set.
#[derive(Clone, Debug, Default)]
pub(crate) struct CachedHookId {
	pub on_hook: Option<ExportId>,
}

const SLOW_CALL_THRESHOLD: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NuDecodeSurface {
	Macro,
	Hook,
}

/// Loaded Nu macro script runtime state.
#[derive(Clone)]
pub struct NuRuntime {
	config_dir: PathBuf,
	script_path: PathBuf,
	program: NuProgram,
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
		let program = NuProgram::compile_macro_from_dir(config_dir).map_err(|error| error.to_string())?;
		let script_path = program.script_path().to_path_buf();
		Ok(Self {
			config_dir: config_dir.to_path_buf(),
			script_path,
			program,
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

	/// Run a macro function and decode its return value into typed effects.
	pub fn run_macro_effects_with_budget_and_env(
		&self,
		fn_name: &str,
		args: &[String],
		budget: DecodeBudget,
		env: &[(&str, Value)],
	) -> Result<NuEffectBatch, String> {
		let value = self.run_internal(fn_name, args, env).map_err(map_run_error)?;
		xeno_invocation::nu::decode_macro_effects_with_budget(value, budget)
	}

	/// Resolve an exported declaration by name. Returns `None` for
	/// missing exports and builtins.
	pub fn find_export(&self, name: &str) -> Option<ExportId> {
		self.program.resolve_export(name)
	}

	/// Run a pre-resolved declaration and decode into typed effects.
	pub fn run_effects_by_decl_id(
		&self,
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: &[String],
		budget: DecodeBudget,
		env: &[(&str, Value)],
	) -> Result<NuEffectBatch, String> {
		let value = self.call_by_decl_id(decl_id, args, env)?;
		decode_effects(surface, value, budget)
	}

	/// Run a pre-resolved declaration with owned args/env (zero-clone hot path)
	/// and decode into typed effects.
	pub fn run_effects_by_decl_id_owned(
		&self,
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: Vec<String>,
		budget: DecodeBudget,
		env: Vec<(String, Value)>,
		host: Option<&(dyn xeno_nu_api::XenoNuHost + 'static)>,
	) -> Result<NuEffectBatch, String> {
		let start = Instant::now();
		let value = self.program.call_export_owned(decl_id, args, env, host).map_err(|error| error.to_string())?;
		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}
		decode_effects(surface, value, budget)
	}

	fn call_by_decl_id(&self, decl_id: ExportId, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
		let start = Instant::now();
		let value = self.program.call_export(decl_id, args, env, None).map_err(|error| error.to_string())?;
		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}
		Ok(value)
	}

	fn run_internal(&self, fn_name: &str, args: &[String], env: &[(&str, Value)]) -> Result<Value, NuRunError> {
		let start = Instant::now();

		let decl_id = self
			.program
			.resolve_export(fn_name)
			.ok_or_else(|| NuRunError::MissingFunction(fn_name.to_string()))?;

		let value = self
			.program
			.call_export(decl_id, args, env, None)
			.map_err(|error| NuRunError::Other(error.to_string()))?;

		let elapsed = start.elapsed();
		if elapsed > SLOW_CALL_THRESHOLD {
			tracing::debug!(function = fn_name, elapsed_ms = elapsed.as_millis() as u64, "slow Nu call");
		}

		Ok(value)
	}
}

fn decode_effects(surface: NuDecodeSurface, value: Value, budget: DecodeBudget) -> Result<NuEffectBatch, String> {
	match surface {
		NuDecodeSurface::Macro => xeno_invocation::nu::decode_macro_effects_with_budget(value, budget),
		NuDecodeSurface::Hook => xeno_invocation::nu::decode_hook_effects_with_budget(value, budget),
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
