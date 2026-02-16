//! Nu program compiler/executor facade for Xeno.
//!
//! This crate exposes a stable split between:
//! * compilation (`NuProgram::compile_*`) under an explicit policy
//! * execution (`NuProgram::call_export*`, `NuProgram::execute_root`)
//!
//! The facade wraps vendored Nu internals used for `xeno.nu` and `config.nu`
//! while enforcing the sandboxed evaluation environment.

mod sandbox;

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use xeno_nu_protocol::DeclId;
use xeno_nu_protocol::ast::Block;
use xeno_nu_protocol::engine::EngineState;
use xeno_nu_value::Value;

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
	/// Module-only scripts used for `xeno.nu` (defs/imports/consts/modules).
	MacroModule,
	/// General scripts used for `config.nu` evaluation.
	ConfigScript,
}

impl ProgramPolicy {
	fn parse_policy(self) -> sandbox::ParsePolicy {
		match self {
			Self::MacroModule => sandbox::ParsePolicy::ModuleOnly,
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

/// Error emitted while executing a compiled [`NuProgram`].
#[derive(Debug, Clone)]
pub enum ExecError {
	MissingExport(String),
	InvalidExportId(usize),
	Runtime(String),
}

impl fmt::Display for ExecError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::MissingExport(message) | Self::Runtime(message) => f.write_str(message),
			Self::InvalidExportId(raw) => write!(f, "Nu runtime error: export id {raw} is not defined in compiled program"),
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
	script_decls: Arc<HashSet<DeclId>>,
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

		Self::compile_source(config_dir, &script_path, &script_src, ProgramPolicy::MacroModule)
	}

	/// Compile a macro module source blob as if it were `xeno.nu`.
	pub fn compile_macro_source(config_dir: &Path, script_path: &Path, script_src: &str) -> Result<Self, CompileError> {
		Self::compile_source(config_dir, script_path, script_src, ProgramPolicy::MacroModule)
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

		Ok(Self {
			policy,
			config_dir: config_dir.map(Path::to_path_buf),
			script_path: script_path.to_path_buf(),
			engine_state: Arc::new(engine_state),
			script_decls: Arc::new(parsed.script_decl_ids.into_iter().collect()),
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
	pub fn resolve_export(&self, name: &str) -> Option<ExportId> {
		let decl_id = sandbox::find_decl(&self.engine_state, name)?;
		self.script_decls.contains(&decl_id).then_some(ExportId::from_decl_id(decl_id))
	}

	/// Call a pre-resolved export.
	pub fn call_export(&self, export: ExportId, args: &[String], env: &[(&str, Value)]) -> Result<Value, ExecError> {
		let decl_id = self.checked_decl_id(export)?;
		sandbox::call_function(&self.engine_state, decl_id, args, env).map_err(ExecError::Runtime)
	}

	/// Call a pre-resolved export with owned args/env.
	pub fn call_export_owned(&self, export: ExportId, args: Vec<String>, env: Vec<(String, Value)>) -> Result<Value, ExecError> {
		let decl_id = self.checked_decl_id(export)?;
		sandbox::call_function_owned(&self.engine_state, decl_id, args, env).map_err(ExecError::Runtime)
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
		sandbox::evaluate_block(&self.engine_state, block.as_ref()).map_err(ExecError::Runtime)
	}

	fn checked_decl_id(&self, export: ExportId) -> Result<DeclId, ExecError> {
		let decl_id = export.to_decl_id();
		if !self.script_decls.contains(&decl_id) {
			return Err(ExecError::InvalidExportId(export.raw()));
		}
		Ok(decl_id)
	}
}

fn add_prelude_removal_hint(error: &str) -> String {
	let lower = error.to_ascii_lowercase();
	if lower.contains("use xeno") || (lower.contains("module") && lower.contains("xeno") && lower.contains("not found")) {
		format!(
			"{error}\n\nHint: the built-in `xeno` prelude module was removed. \
\t\t Delete `use xeno *` and call built-in commands directly: \
\t\t xeno emit, xeno emit-many, xeno call, xeno ctx."
		)
	} else {
		error.to_string()
	}
}

#[cfg(test)]
mod tests;
