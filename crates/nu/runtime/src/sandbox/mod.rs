//! Sandboxed Nu evaluation environment.
//!
//! # Security model
//!
//! Sandboxing is composed from three layers:
//!
//! * **Engine context allowlist** — a minimal `nu-cmd-lang` context (no
//!   filesystem/network/plugin command sets) with dangerous language commands
//!   excluded (`for`/`while`/`loop`, overlay commands, external signatures).
//!   Safe stdlib commands are registered from `xeno-nu-safe-commands`.
//! * **AST-level scan** (`scan.rs`) — rejects external commands (`^cmd`),
//!   pipeline redirection, glob expansion, filepath/directory literals,
//!   range expressions (unbounded iteration), and defense-in-depth
//!   `source`/`source-env` rejection.
//! * **Module root confinement** — all parser-resolved module files must
//!   canonically remain under the provided config root directory.
//!
//! # Call input caps
//!
//! Function calls are subject to hard limits from
//! [`xeno_invocation::nu::DEFAULT_CALL_LIMITS`] to prevent resource exhaustion.
//! Limits are derived from [`xeno_invocation::schema::DEFAULT_LIMITS`] where
//! applicable (args, string lengths).
//!
//! # Recursion limit
//!
//! Nu engine recursion is capped at 64 frames.
//!
//! # Safe stdlib allowlist
//!
//! The following commands are registered from `xeno-nu-safe-commands`:
//!
//! Filters: `append`, `compact`, `each`, `flatten`, `get`, `is-empty`,
//! `length`, `prepend`, `reduce`, `reject`, `select`, `sort` (`--nulls-first`),
//! `sort-by` (simple columns, `--nulls-first`), `update`, `upsert`, `where`
//!
//! Strings: `split row` (literal-only), `str contains`, `str downcase`,
//! `str ends-with`, `str replace` (literal-only), `str starts-with`,
//! `str trim`, `str upcase`
//!
//! Conversions: `into int`, `into bool`, `into string` (simple column mode
//! supported)
//!
//! Builtins (from `commands/`): `xeno call`, `xeno assert`
//! (validation gate; errors abort evaluation), `xeno ctx`,
//! `xeno effect` (typed effect constructor),
//! `xeno effects normalize` (bulk validate/normalize typed effects),
//! `xeno is-effect` (predicate: true if input decodes as a single effect),
//! `xeno log` (pass-through pipeline logger)
//!
//! Caveats:
//! * `split row --regex` and `str replace --regex` are disabled (no
//!   regex engine in the sandbox).
//! * `str trim`, `str replace`, `split row` support simple column names
//!   (e.g. `str trim name`) for record/table input; complex cell paths
//!   are rejected.
//! * `str contains` doesn't support cell-path traversal.
//! * `select` is pure record projection (no SQLite/stream paths).
//! * `length` doesn't support SQLite streams.
//!
//! # Safe stdlib limits
//!
//! All iteration commands cap at 10 000 items (`MAX_ITEMS`).
//! Projection commands cap at 128 columns (`MAX_COLUMNS`).
//! `split row` caps at 10 000 segments per value (`MAX_SPLITS`).

pub(crate) mod commands;
mod scan;

use std::path::Path;
use std::sync::Arc;

pub(crate) use scan::ensure_sandboxed;
use xeno_nu_protocol::ast::{Block, Expr, Expression};
use xeno_nu_protocol::config::Config;
use xeno_nu_protocol::debugger::WithoutDebug;
use xeno_nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use xeno_nu_protocol::{DeclId, PipelineData, Span, Type, Value};

use crate::CallValidationError;

const XENO_NU_RECURSION_LIMIT: i64 = 64;

/// Error from sandbox call execution: either input validation or Nu engine.
#[derive(Debug)]
pub(crate) enum SandboxCallError {
	Validation(CallValidationError),
	Runtime(String),
}

impl From<CallValidationError> for SandboxCallError {
	fn from(e: CallValidationError) -> Self {
		Self::Validation(e)
	}
}

use xeno_invocation::nu::DEFAULT_CALL_LIMITS;

/// Creates a minimal Nu engine state suitable for sandboxed evaluation.
pub(crate) fn create_engine_state(config_root: Option<&Path>) -> Result<EngineState, String> {
	let mut engine_state = create_xeno_lang_context()?;
	let mut config: Config = engine_state.get_config().as_ref().clone();
	config.recursion_limit = XENO_NU_RECURSION_LIMIT;
	engine_state.set_config(config);

	if let Some(cwd) = config_root.and_then(|p| std::fs::canonicalize(p).ok()) {
		engine_state.add_env_var("PWD".to_string(), Value::string(cwd.to_string_lossy().to_string(), Span::unknown()));
	}

	register_xeno_commands(&mut engine_state)?;
	Ok(engine_state)
}

fn register_xeno_commands(engine_state: &mut EngineState) -> Result<(), String> {
	let delta = {
		let mut working_set = StateWorkingSet::new(engine_state);
		commands::register_all(&mut working_set);
		working_set.render()
	};
	engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;
	Ok(())
}

fn create_xeno_lang_context() -> Result<EngineState, String> {
	let mut engine_state = EngineState::new();
	let delta = {
		let mut working_set = StateWorkingSet::new(&engine_state);
		macro_rules! bind {
			( $( $cmd:expr ),* $(,)? ) => {
				$( working_set.add_decl(Box::new($cmd)); )*
			};
		}
		bind! {
			xeno_nu_cmd_lang::Def,
			xeno_nu_cmd_lang::ExportDef,
			xeno_nu_cmd_lang::Module,
			xeno_nu_cmd_lang::ExportModule,
			xeno_nu_cmd_lang::Use,
			xeno_nu_cmd_lang::ExportUse,
			xeno_nu_cmd_lang::Let,
			xeno_nu_cmd_lang::Mut,
			xeno_nu_cmd_lang::Const,
			xeno_nu_cmd_lang::ExportConst,
			xeno_nu_cmd_lang::If,
			xeno_nu_cmd_lang::Match,
			xeno_nu_cmd_lang::Do,
			xeno_nu_cmd_lang::Try,
			xeno_nu_cmd_lang::Return,
			xeno_nu_cmd_lang::Echo,
			xeno_nu_cmd_lang::Error,
			xeno_nu_cmd_lang::ErrorMake,
		}
		xeno_nu_safe_commands::register_all(&mut working_set);
		working_set.render()
	};
	engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;
	Ok(engine_state)
}

/// Result of parsing and validating a Nu script.
#[derive(Debug)]
pub(crate) struct ParseResult {
	pub block: Arc<Block>,
	/// Decl IDs of exported definitions (from module export table).
	/// Empty for `Script` policy (config scripts have no exports).
	pub export_decl_ids: Vec<DeclId>,
}

/// Controls what top-level constructs are allowed in a parsed script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsePolicy {
	/// Allow any expression at top level (used by `config.nu`).
	Script,
	/// Wraps source as `module __xeno__ { <source> }; use __xeno__ *` to
	/// enforce module-only constructs and extract proper exports. Used by `xeno.nu`.
	ModuleWrapped,
}

/// Parses Nu source with default Script policy.
#[cfg(test)]
pub(crate) fn parse_and_validate(engine_state: &mut EngineState, fname: &str, source: &str, config_root: Option<&Path>) -> Result<ParseResult, String> {
	parse_and_validate_with_policy(engine_state, fname, source, config_root, ParsePolicy::Script)
}

/// Parses Nu source, validates the sandbox and parse policy, and merges into
/// the engine state.
pub(crate) fn parse_and_validate_with_policy(
	engine_state: &mut EngineState,
	fname: &str,
	source: &str,
	config_root: Option<&Path>,
	policy: ParsePolicy,
) -> Result<ParseResult, String> {
	let (block, export_decl_ids) = if policy == ParsePolicy::ModuleWrapped {
		// Parse as a module to get proper export semantics: wrap source in
		// `module __xeno__ { <source> }; use __xeno__ *` so that only `export def`
		// and re-exports via `export use` are visible at top level.
		let wrapped = format!("module __xeno__ {{\n{source}\n}}\nuse __xeno__ *");
		let mut working_set = StateWorkingSet::new(engine_state);
		let base_decls = working_set.permanent_state.num_decls();

		let block = xeno_nu_parser::parse(&mut working_set, Some(fname), wrapped.as_bytes(), false);

		if let Some(error) = working_set.parse_errors.first() {
			return Err(format!("Nu parse error: {error}"));
		}
		if let Some(error) = working_set.compile_errors.first() {
			return Err(format!("Nu compile error: {error}"));
		}

		ensure_sandboxed(&working_set, block.as_ref(), config_root)?;

		let added_decls = working_set.delta.num_decls();
		let script_decl_ids: Vec<DeclId> = (0..added_decls).map(|i| DeclId::new(base_decls + i)).collect();
		check_reserved_names(&working_set, &script_decl_ids)?;

		// Find the __xeno__ module and extract its export table.
		let module_id = working_set.find_module(b"__xeno__");
		let export_decl_ids = if let Some(module_id) = module_id {
			let module = working_set.get_module(module_id);
			module.decls.values().copied().collect()
		} else {
			Vec::new()
		};

		let delta = working_set.render();
		engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;

		(block, export_decl_ids)
	} else {
		// Script policy: parse as standalone, no exports.
		let mut working_set = StateWorkingSet::new(engine_state);
		let block = xeno_nu_parser::parse(&mut working_set, Some(fname), source.as_bytes(), false);

		if let Some(error) = working_set.parse_errors.first() {
			return Err(format!("Nu parse error: {error}"));
		}
		if let Some(error) = working_set.compile_errors.first() {
			return Err(format!("Nu compile error: {error}"));
		}

		ensure_sandboxed(&working_set, block.as_ref(), config_root)?;

		let delta = working_set.render();
		engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;

		(block, Vec::new())
	};

	Ok(ParseResult { block, export_decl_ids })
}

fn is_reserved_xeno_name(name: &str) -> bool {
	name == "xeno" || name.starts_with("xeno ")
}

fn check_reserved_names(working_set: &StateWorkingSet<'_>, script_decl_ids: &[DeclId]) -> Result<(), String> {
	for &decl_id in script_decl_ids {
		let name = working_set.get_decl(decl_id).name();
		if is_reserved_xeno_name(name) {
			return Err(format!(
				"Nu script error: '{name}' is in the reserved 'xeno' command namespace; rename your definition"
			));
		}
	}
	Ok(())
}

/// Evaluates a parsed block and returns the resulting value.
pub(crate) fn evaluate_block(engine_state: &EngineState, block: &Block) -> Result<Value, String> {
	let mut stack = Stack::new();
	let eval_block = xeno_nu_engine::get_eval_block(engine_state);
	let execution = eval_block(engine_state, &mut stack, block, PipelineData::empty()).map_err(|error| format!("Nu runtime error: {error}"))?;
	execution.body.into_value(Span::unknown()).map_err(|error| format!("Nu runtime error: {error}"))
}

/// Calls an already-registered function by declaration ID.
pub(crate) fn call_function(engine_state: &EngineState, decl_id: DeclId, args: &[String], env: &[(&str, Value)]) -> Result<Value, SandboxCallError> {
	validate_call_args(args)?;
	validate_call_env_borrowed(env)?;

	let span = Span::unknown();
	let mut call = resolve_decl_call(decl_id, span);
	for arg in args {
		call.add_positional(Expression::new_unknown(Expr::String(arg.clone()), span, Type::String));
	}

	let mut stack = Stack::new();
	for (key, value) in env {
		stack.add_env_var((*key).to_string(), value.clone());
	}

	let result = xeno_nu_engine::eval_call::<WithoutDebug>(engine_state, &mut stack, &call, PipelineData::empty())
		.map_err(|error| SandboxCallError::Runtime(format!("Nu runtime error: {error}")))?;
	result
		.into_value(span)
		.map_err(|error| SandboxCallError::Runtime(format!("Nu runtime error: {error}")))
}

/// Like [`call_function`] but consumes owned args and env.
pub(crate) fn call_function_owned(
	engine_state: &EngineState,
	decl_id: DeclId,
	args: Vec<String>,
	env: Vec<(String, Value)>,
) -> Result<Value, SandboxCallError> {
	validate_call_args(&args)?;
	validate_call_env_owned(&env)?;

	let span = Span::unknown();
	let mut call = resolve_decl_call(decl_id, span);
	for arg in args {
		call.add_positional(Expression::new_unknown(Expr::String(arg), span, Type::String));
	}

	let mut stack = Stack::new();
	for (key, value) in env {
		stack.add_env_var(key, value);
	}

	let result = xeno_nu_engine::eval_call::<WithoutDebug>(engine_state, &mut stack, &call, PipelineData::empty())
		.map_err(|error| SandboxCallError::Runtime(format!("Nu runtime error: {error}")))?;
	result
		.into_value(span)
		.map_err(|error| SandboxCallError::Runtime(format!("Nu runtime error: {error}")))
}

fn resolve_decl_call(decl_id: DeclId, span: Span) -> xeno_nu_protocol::ast::Call {
	let mut call = xeno_nu_protocol::ast::Call::new(span);
	call.decl_id = decl_id;
	call
}

/// Looks up a declaration by name in the engine state.
#[cfg(test)]
pub(crate) fn find_decl(engine_state: &EngineState, name: &str) -> Option<DeclId> {
	engine_state.find_decl(name.as_bytes(), &[])
}

// ---------------------------------------------------------------------------
// Input validation for function calls
// ---------------------------------------------------------------------------

fn validate_call_args(args: &[String]) -> Result<(), CallValidationError> {
	if args.len() > DEFAULT_CALL_LIMITS.max_args {
		return Err(CallValidationError::ArgsTooMany {
			len: args.len(),
			max: DEFAULT_CALL_LIMITS.max_args,
		});
	}
	for (idx, arg) in args.iter().enumerate() {
		if arg.len() > DEFAULT_CALL_LIMITS.max_arg_len {
			return Err(CallValidationError::ArgTooLong {
				idx,
				len: arg.len(),
				max: DEFAULT_CALL_LIMITS.max_arg_len,
			});
		}
	}
	Ok(())
}

fn validate_call_env_borrowed(env: &[(&str, Value)]) -> Result<(), CallValidationError> {
	if env.len() > DEFAULT_CALL_LIMITS.max_env_vars {
		return Err(CallValidationError::EnvTooMany {
			len: env.len(),
			max: DEFAULT_CALL_LIMITS.max_env_vars,
		});
	}
	let mut nodes = 0usize;
	for (key, value) in env {
		if key.len() > DEFAULT_CALL_LIMITS.max_env_string_len {
			return Err(CallValidationError::EnvKeyTooLong {
				len: key.len(),
				max: DEFAULT_CALL_LIMITS.max_env_string_len,
			});
		}
		count_value_nodes(value, &mut nodes)?;
	}
	Ok(())
}

fn validate_call_env_owned(env: &[(String, Value)]) -> Result<(), CallValidationError> {
	if env.len() > DEFAULT_CALL_LIMITS.max_env_vars {
		return Err(CallValidationError::EnvTooMany {
			len: env.len(),
			max: DEFAULT_CALL_LIMITS.max_env_vars,
		});
	}
	let mut nodes = 0usize;
	for (key, value) in env {
		if key.len() > DEFAULT_CALL_LIMITS.max_env_string_len {
			return Err(CallValidationError::EnvKeyTooLong {
				len: key.len(),
				max: DEFAULT_CALL_LIMITS.max_env_string_len,
			});
		}
		count_value_nodes(value, &mut nodes)?;
	}
	Ok(())
}

fn count_value_nodes(value: &Value, nodes: &mut usize) -> Result<(), CallValidationError> {
	*nodes += 1;
	if *nodes > DEFAULT_CALL_LIMITS.max_env_nodes {
		return Err(CallValidationError::EnvValueTooComplex {
			nodes: *nodes,
			max: DEFAULT_CALL_LIMITS.max_env_nodes,
		});
	}
	match value {
		Value::String { val, .. } => {
			if val.len() > DEFAULT_CALL_LIMITS.max_env_string_len {
				return Err(CallValidationError::EnvStringTooLong {
					len: val.len(),
					max: DEFAULT_CALL_LIMITS.max_env_string_len,
				});
			}
		}
		Value::List { vals, .. } => {
			for v in vals {
				count_value_nodes(v, nodes)?;
			}
		}
		Value::Record { val, .. } => {
			for (k, v) in val.iter() {
				if k.len() > DEFAULT_CALL_LIMITS.max_env_string_len {
					return Err(CallValidationError::EnvKeyTooLong {
						len: k.len(),
						max: DEFAULT_CALL_LIMITS.max_env_string_len,
					});
				}
				count_value_nodes(v, nodes)?;
			}
		}
		_ => {}
	}
	Ok(())
}

#[cfg(test)]
mod tests;
