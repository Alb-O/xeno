//! Nu program compiler/executor facade for Xeno.
//!
//! This crate exposes a stable split between:
//! * compilation (`NuProgram::compile_*`) under an explicit policy
//! * execution (`NuProgram::call_export*`, `NuProgram::execute_root`)
//!
//! The facade wraps vendored Nu internals used for `xeno.nu` and `config.nu`
//! while enforcing the sandboxed evaluation environment.
#![allow(clippy::result_large_err, reason = "ShellError is intentionally rich and shared across Nu runtime APIs")]

mod sandbox;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use xeno_nu_data::Value;
use xeno_nu_protocol::ast::Block;
use xeno_nu_protocol::engine::EngineState;
use xeno_nu_protocol::{DeclId, Value as ProtocolValue};

const SCRIPT_FILE_NAME: &str = "xeno.nu";

/// Hard limit on script/source size to prevent DoS via pathological input.
const MAX_SCRIPT_BYTES: usize = 512 * 1024;

/// Stable identifier for a compiled Nu export declaration.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExportId(usize);

impl ExportId {
	pub const fn from_raw(raw: usize) -> Self {
		Self(raw)
	}

	pub const fn raw(self) -> usize {
		self.0
	}

	fn from_decl_id(decl_id: DeclId) -> Self {
		Self(decl_id.get())
	}

	fn to_decl_id(self) -> DeclId {
		DeclId::new(self.0)
	}
}

impl fmt::Debug for ExportId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "ExportId({})", self.0)
	}
}

/// Compilation policy describing allowed top-level constructs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramPolicy {
	/// Source is wrapped as `module __xeno__ { <source> }; use __xeno__ *`
	/// so only `export def` and re-exports are visible. Used for `xeno.nu`.
	ModuleWrapped,
	/// General scripts used for `config.nu` evaluation.
	ConfigScript,
}

impl ProgramPolicy {
	fn parse_policy(self) -> sandbox::ParsePolicy {
		match self {
			Self::ModuleWrapped => sandbox::ParsePolicy::ModuleWrapped,
			Self::ConfigScript => sandbox::ParsePolicy::Script,
		}
	}
}

/// Error emitted while compiling Nu source into a [`NuProgram`].
#[derive(Debug, Clone)]
pub enum CompileError {
	Io(String),
	Parse(String),
}

impl fmt::Display for CompileError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Io(message) | Self::Parse(message) => f.write_str(message),
		}
	}
}

impl Error for CompileError {}

/// Structured call validation failure.
///
/// Returned when inputs to a Nu function call exceed configured limits
/// (from [`xeno_invocation::nu::DEFAULT_CALL_LIMITS`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallValidationError {
	ArgsTooMany { len: usize, max: usize },
	ArgTooLong { idx: usize, len: usize, max: usize },
	EnvTooMany { len: usize, max: usize },
	EnvKeyTooLong { len: usize, max: usize },
	EnvValueTooComplex { nodes: usize, max: usize },
	EnvStringTooLong { len: usize, max: usize },
}

impl fmt::Display for CallValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::ArgsTooMany { len, max } => write!(f, "Nu call error: {len} args exceeds limit of {max}"),
			Self::ArgTooLong { idx, len, max } => write!(f, "Nu call error: arg[{idx}] length {len} exceeds limit of {max}"),
			Self::EnvTooMany { len, max } => write!(f, "Nu call error: {len} env vars exceeds limit of {max}"),
			Self::EnvKeyTooLong { len, max } => write!(f, "Nu call error: env key length {len} exceeds limit of {max}"),
			Self::EnvValueTooComplex { nodes, max } => write!(f, "Nu call error: env value traversal ({nodes} nodes) exceeds limit of {max}"),
			Self::EnvStringTooLong { len, max } => write!(f, "Nu call error: env string length {len} exceeds limit of {max}"),
		}
	}
}

/// Error emitted while executing a compiled [`NuProgram`].
#[derive(Debug, Clone)]
pub enum ExecError {
	MissingExport(String),
	InvalidExportId(usize),
	CallValidation(CallValidationError),
	Runtime(String),
}

impl fmt::Display for ExecError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::MissingExport(message) | Self::Runtime(message) => f.write_str(message),
			Self::InvalidExportId(raw) => write!(f, "Nu runtime error: export id {raw} is not defined in compiled program"),
			Self::CallValidation(err) => write!(f, "{err}"),
		}
	}
}

impl Error for ExecError {}

/// Compiled Nu program plus execution metadata.
#[derive(Clone)]
pub struct NuProgram {
	policy: ProgramPolicy,
	config_dir: Option<PathBuf>,
	script_path: PathBuf,
	engine_state: Arc<EngineState>,
	/// Only explicitly exported decls (`export def`). Used for resolve/call gating.
	export_decls: Arc<HashSet<DeclId>>,
	/// Export name → DeclId lookup for `resolve_export`.
	export_names: Arc<HashMap<String, DeclId>>,
	root_block: Option<Arc<Block>>,
}

impl fmt::Debug for NuProgram {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NuProgram")
			.field("policy", &self.policy)
			.field("config_dir", &self.config_dir)
			.field("script_path", &self.script_path)
			.finish_non_exhaustive()
	}
}

impl NuProgram {
	/// Compile `xeno.nu` from a config directory with macro-module policy.
	pub fn compile_macro_from_dir(config_dir: &Path) -> Result<Self, CompileError> {
		let script_path = config_dir.join(SCRIPT_FILE_NAME);
		let metadata = std::fs::metadata(&script_path).map_err(|error| CompileError::Io(format!("failed to read {}: {error}", script_path.display())))?;
		if metadata.len() as usize > MAX_SCRIPT_BYTES {
			return Err(CompileError::Parse(format!("Nu runtime error: script exceeds {} byte limit", MAX_SCRIPT_BYTES)));
		}

		let script_src =
			std::fs::read_to_string(&script_path).map_err(|error| CompileError::Io(format!("failed to read {}: {error}", script_path.display())))?;

		Self::compile_source(config_dir, &script_path, &script_src, ProgramPolicy::ModuleWrapped)
	}

	/// Compile a macro module source blob as if it were `xeno.nu`.
	pub fn compile_macro_source(config_dir: &Path, script_path: &Path, script_src: &str) -> Result<Self, CompileError> {
		Self::compile_source(config_dir, script_path, script_src, ProgramPolicy::ModuleWrapped)
	}

	/// Compile a config script with script policy.
	pub fn compile_config_script(fname: &str, source: &str, config_root: Option<&Path>) -> Result<Self, CompileError> {
		let script_path = PathBuf::from(fname);
		let root = config_root.map(Path::to_path_buf);
		Self::compile_source_opt(root.as_deref(), &script_path, source, ProgramPolicy::ConfigScript)
	}

	/// Compile source using an explicit policy.
	pub fn compile_source(config_dir: &Path, script_path: &Path, source: &str, policy: ProgramPolicy) -> Result<Self, CompileError> {
		Self::compile_source_opt(Some(config_dir), script_path, source, policy)
	}

	fn compile_source_opt(config_dir: Option<&Path>, script_path: &Path, source: &str, policy: ProgramPolicy) -> Result<Self, CompileError> {
		if source.len() > MAX_SCRIPT_BYTES {
			return Err(CompileError::Parse(format!("Nu runtime error: script exceeds {} byte limit", MAX_SCRIPT_BYTES)));
		}

		let mut engine_state = sandbox::create_engine_state(config_dir).map_err(CompileError::Parse)?;
		let fname = script_path.to_string_lossy().to_string();
		let parsed = sandbox::parse_and_validate_with_policy(&mut engine_state, &fname, source, config_dir, policy.parse_policy())
			.map_err(|e| CompileError::Parse(add_prelude_removal_hint(&e)))?;

		let root_block = (policy == ProgramPolicy::ConfigScript).then_some(parsed.block.clone());

		let export_decl_set: HashSet<DeclId> = parsed.export_decl_ids.iter().copied().collect();
		let export_name_map: HashMap<String, DeclId> = parsed
			.export_decl_ids
			.iter()
			.map(|&id| {
				let name = engine_state.get_decl(id).name().to_string();
				(name, id)
			})
			.collect();

		Ok(Self {
			policy,
			config_dir: config_dir.map(Path::to_path_buf),
			script_path: script_path.to_path_buf(),
			engine_state: Arc::new(engine_state),
			export_decls: Arc::new(export_decl_set),
			export_names: Arc::new(export_name_map),
			root_block,
		})
	}

	/// Returns the policy used to compile this program.
	pub fn policy(&self) -> ProgramPolicy {
		self.policy
	}

	/// Returns the source path used for diagnostics.
	pub fn script_path(&self) -> &Path {
		&self.script_path
	}

	/// Resolve an export declaration by name.
	///
	/// Only returns explicitly exported definitions (`export def`).
	/// Private helpers (`def`) are not resolvable.
	pub fn resolve_export(&self, name: &str) -> Option<ExportId> {
		self.export_names.get(name).map(|&id| ExportId::from_decl_id(id))
	}

	/// Call a pre-resolved export.
	pub fn call_export(&self, export: ExportId, args: &[String], env: &[(&str, Value)]) -> Result<Value, ExecError> {
		let decl_id = self.checked_decl_id(export)?;
		let env = env.iter().map(|(key, value)| (*key, ProtocolValue::from(value.clone()))).collect::<Vec<_>>();
		let value = sandbox::call_function(&self.engine_state, decl_id, args, &env).map_err(map_sandbox_err)?;
		Value::try_from(value).map_err(|error| ExecError::Runtime(format!("Nu runtime error: {error}")))
	}

	/// Call a pre-resolved export with owned args/env.
	pub fn call_export_owned(&self, export: ExportId, args: Vec<String>, env: Vec<(String, Value)>) -> Result<Value, ExecError> {
		let decl_id = self.checked_decl_id(export)?;
		let env = env.into_iter().map(|(key, value)| (key, ProtocolValue::from(value))).collect::<Vec<_>>();
		let value = sandbox::call_function_owned(&self.engine_state, decl_id, args, env).map_err(map_sandbox_err)?;
		Value::try_from(value).map_err(|error| ExecError::Runtime(format!("Nu runtime error: {error}")))
	}

	/// Resolve and call an export by name.
	pub fn call_export_name(&self, name: &str, args: &[String], env: &[(&str, Value)]) -> Result<Value, ExecError> {
		let export = self
			.resolve_export(name)
			.ok_or_else(|| ExecError::MissingExport(format!("Nu runtime error: function '{name}' is not defined in xeno.nu")))?;
		self.call_export(export, args, env)
	}

	/// Execute the script root block (config policy programs only).
	pub fn execute_root(&self) -> Result<Value, ExecError> {
		let Some(block) = self.root_block.as_ref() else {
			return Err(ExecError::Runtime(
				"Nu runtime error: execute_root is only available for config-script programs".to_string(),
			));
		};
		let value = sandbox::evaluate_block(&self.engine_state, block.as_ref()).map_err(ExecError::Runtime)?;
		Value::try_from(value).map_err(|error| ExecError::Runtime(format!("Nu runtime error: {error}")))
	}

	/// Returns all exported definitions, sorted by name.
	pub fn exports(&self) -> Vec<(String, ExportId)> {
		let mut out: Vec<_> = self.export_names.iter().map(|(name, &id)| (name.clone(), ExportId::from_decl_id(id))).collect();
		out.sort_by(|a, b| a.0.cmp(&b.0));
		out
	}

	fn checked_decl_id(&self, export: ExportId) -> Result<DeclId, ExecError> {
		let decl_id = export.to_decl_id();
		if !self.export_decls.contains(&decl_id) {
			return Err(ExecError::InvalidExportId(export.raw()));
		}
		Ok(decl_id)
	}
}

fn map_sandbox_err(err: sandbox::SandboxCallError) -> ExecError {
	match err {
		sandbox::SandboxCallError::Validation(v) => ExecError::CallValidation(v),
		sandbox::SandboxCallError::Runtime(msg) => ExecError::Runtime(msg),
	}
}

fn add_prelude_removal_hint(error: &str) -> String {
	let lower = error.to_ascii_lowercase();
	if lower.contains("use xeno") || (lower.contains("module") && lower.contains("xeno") && lower.contains("not found")) {
		format!(
			"{error}\n\nHint: the built-in `xeno` prelude module was removed. \
\t\t Delete `use xeno *` and call built-in commands directly: \
\t\t xeno effect, xeno effects normalize, xeno call, xeno ctx."
		)
	} else {
		error.to_string()
	}
}

#[cfg(test)]
mod tests;
