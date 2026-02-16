//! Nu runtime facade for Xeno.
//!
//! Provides a stable API around the vendored Nu internals used for `xeno.nu`
//! and `config.nu` evaluation. Includes the sandboxed evaluation environment.

mod sandbox;

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use sandbox::ParsePolicy;
use xeno_nu_protocol::DeclId;
use xeno_nu_protocol::engine::EngineState;
use xeno_nu_value::Value;

const SCRIPT_FILE_NAME: &str = "xeno.nu";

/// Hard limit on script/source size to prevent DoS via pathological input.
const MAX_SCRIPT_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(usize);

impl FunctionId {
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

impl fmt::Debug for FunctionId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "FunctionId({})", self.0)
	}
}

#[derive(Clone)]
pub struct Runtime {
	config_dir: PathBuf,
	script_path: PathBuf,
	engine_state: Arc<EngineState>,
	script_decls: Arc<HashSet<DeclId>>,
}

impl fmt::Debug for Runtime {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Runtime")
			.field("config_dir", &self.config_dir)
			.field("script_path", &self.script_path)
			.finish_non_exhaustive()
	}
}

impl Runtime {
	pub fn load(config_dir: &Path) -> Result<Self, String> {
		let script_path = config_dir.join(SCRIPT_FILE_NAME);
		let metadata = std::fs::metadata(&script_path).map_err(|error| format!("failed to read {}: {error}", script_path.display()))?;
		if metadata.len() as usize > MAX_SCRIPT_BYTES {
			return Err(format!("Nu runtime error: script exceeds {} byte limit", MAX_SCRIPT_BYTES));
		}
		let script_src = std::fs::read_to_string(&script_path).map_err(|error| format!("failed to read {}: {error}", script_path.display()))?;
		Self::load_source(config_dir, &script_path, &script_src)
	}

	pub fn load_source(config_dir: &Path, script_path: &Path, script_src: &str) -> Result<Self, String> {
		if script_src.len() > MAX_SCRIPT_BYTES {
			return Err(format!("Nu runtime error: script exceeds {} byte limit", MAX_SCRIPT_BYTES));
		}
		let mut engine_state = sandbox::create_engine_state(Some(config_dir))?;
		let fname = script_path.to_string_lossy().to_string();
		let parsed = sandbox::parse_and_validate_with_policy(&mut engine_state, &fname, script_src, Some(config_dir), ParsePolicy::ModuleOnly)
			.map_err(|e| add_prelude_removal_hint(&e))?;

		Ok(Self {
			config_dir: config_dir.to_path_buf(),
			script_path: script_path.to_path_buf(),
			engine_state: Arc::new(engine_state),
			script_decls: Arc::new(parsed.script_decl_ids.into_iter().collect()),
		})
	}

	pub fn script_path(&self) -> &Path {
		&self.script_path
	}

	pub fn resolve_function(&self, name: &str) -> Option<FunctionId> {
		let decl_id = sandbox::find_decl(&self.engine_state, name)?;
		self.script_decls.contains(&decl_id).then_some(FunctionId::from_decl_id(decl_id))
	}

	pub fn call(&self, function: FunctionId, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
		let decl_id = self.checked_decl_id(function)?;
		sandbox::call_function(&self.engine_state, decl_id, args, env)
	}

	pub fn call_owned(&self, function: FunctionId, args: Vec<String>, env: Vec<(String, Value)>) -> Result<Value, String> {
		let decl_id = self.checked_decl_id(function)?;
		sandbox::call_function_owned(&self.engine_state, decl_id, args, env)
	}

	pub fn run_function(&self, name: &str, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
		let function = self
			.resolve_function(name)
			.ok_or_else(|| format!("Nu runtime error: function '{name}' is not defined in xeno.nu"))?;
		self.call(function, args, env)
	}

	fn checked_decl_id(&self, function: FunctionId) -> Result<DeclId, String> {
		let decl_id = function.to_decl_id();
		if !self.script_decls.contains(&decl_id) {
			return Err(format!("Nu runtime error: function id {} is not defined in xeno.nu", function.raw()));
		}
		Ok(decl_id)
	}
}

#[derive(Debug, Clone)]
pub enum EvalError {
	Parse(String),
	Runtime(String),
}

impl fmt::Display for EvalError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Parse(message) | Self::Runtime(message) => f.write_str(message),
		}
	}
}

impl Error for EvalError {}

pub fn eval_source_with_policy(fname: &str, source: &str, config_root: Option<&Path>, policy: ParsePolicy) -> Result<Value, EvalError> {
	if source.len() > MAX_SCRIPT_BYTES {
		return Err(EvalError::Parse(format!("Nu runtime error: script exceeds {} byte limit", MAX_SCRIPT_BYTES)));
	}
	let mut engine_state = sandbox::create_engine_state(config_root).map_err(EvalError::Parse)?;
	let parsed = sandbox::parse_and_validate_with_policy(&mut engine_state, fname, source, config_root, policy).map_err(EvalError::Parse)?;
	sandbox::evaluate_block(&engine_state, parsed.block.as_ref()).map_err(EvalError::Runtime)
}

pub fn eval_source(fname: &str, source: &str, config_root: Option<&Path>) -> Result<Value, EvalError> {
	eval_source_with_policy(fname, source, config_root, ParsePolicy::Script)
}

fn add_prelude_removal_hint(error: &str) -> String {
	let lower = error.to_ascii_lowercase();
	if lower.contains("use xeno") || (lower.contains("module") && lower.contains("xeno") && lower.contains("not found")) {
		format!(
			"{error}\n\nHint: the built-in `xeno` prelude module was removed. \
\t\t Delete `use xeno *` and call built-in commands directly: \
\t\t action, command, editor, \"nu run\", \"xeno ctx\"."
		)
	} else {
		error.to_string()
	}
}

#[cfg(test)]
mod tests;
