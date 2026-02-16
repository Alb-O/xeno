//! Shared Nu script sandbox and evaluation helpers for Xeno.
//!
//! Provides a sandboxed Nu evaluation environment used by both the config
//! system (`config.nu`) and the editor macro runtime (`xeno.nu`).
//!
//! Sandboxing is composed from:
//! * A minimal `nu-cmd-lang` engine context (no filesystem/network/plugin
//!   command sets loaded) with dangerous language commands excluded
//!   (`for`/`while`/`loop`, overlay commands, and external signatures).
//! * AST-level policy checks for external execution, redirection/glob usage,
//!   defense-in-depth `source`/`source-env` rejection, and structural escapes.
//! * Post-parse module root enforcement: all parser-resolved module files must
//!   canonically remain under the provided config root.
//!
//! Primary sources:
//! * Nu default language context (`nu-cmd-lang`):
//!   <https://github.com/nushell/nushell/blob/main/crates/nu-cmd-lang/src/default_context.rs>
//! * Nu parser module loading and `source` keyword behavior:
//!   <https://github.com/nushell/nushell/blob/main/crates/nu-parser/src/parse_keywords.rs>
//! * Nu parser import patterns:
//!   <https://github.com/nushell/nushell/blob/main/crates/nu-parser/src/parser.rs>

pub mod commands;
mod sandbox;

use std::path::Path;
use std::sync::Arc;

use xeno_nu_protocol::ast::{Block, Call, Expr, Expression};
use xeno_nu_protocol::config::Config;
use xeno_nu_protocol::debugger::WithoutDebug;
use xeno_nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use xeno_nu_protocol::{DeclId, PipelineData, Span, Type, Value};

pub use crate::sandbox::ensure_sandboxed;

const XENO_NU_RECURSION_LIMIT: i64 = 64;

/// Creates a minimal Nu engine state suitable for sandboxed evaluation.
///
/// Registers only safe language commands — definitions, modules, bindings,
/// control flow, and basic output. Dangerous constructs (loops, overlays,
/// external signatures) are excluded at the engine level so they fail at
/// parse/compile time rather than requiring post-parse sandbox rejection.
///
/// Sets `PWD` to the canonicalized `config_root` if provided.
pub fn create_engine_state(config_root: Option<&Path>) -> Result<EngineState, String> {
	let mut engine_state = create_xeno_lang_context();
	let mut config: Config = engine_state.get_config().as_ref().clone();
	config.recursion_limit = XENO_NU_RECURSION_LIMIT;
	engine_state.set_config(config);

	if let Some(cwd) = config_root.and_then(|p| std::fs::canonicalize(p).ok()) {
		engine_state.add_env_var("PWD".to_string(), Value::string(cwd.to_string_lossy().to_string(), Span::unknown()));
	}

	register_xeno_commands(&mut engine_state);
	Ok(engine_state)
}

/// Registers built-in xeno invocation commands (`action`, `command`, `editor`,
/// `nu run`) into the engine state so they are available in user scripts.
fn register_xeno_commands(engine_state: &mut EngineState) {
	let delta = {
		let mut working_set = StateWorkingSet::new(engine_state);
		commands::register_all(&mut working_set);
		working_set.render()
	};
	engine_state.merge_delta(delta).expect("merge xeno commands");
}

/// Builds a restricted Nu engine context with only safe language commands.
///
/// Excludes from `nu-cmd-lang`'s default context:
/// * Loops: `for`, `while`, `loop`, `break`, `continue`
/// * External signatures: `extern`, `export extern`
/// * Overlays: `overlay`, `overlay use`, `overlay new`, `overlay hide`, `overlay list`
/// * Introspection/debug: `describe`, `scope *`, `version`
/// * Module hygiene: `hide`, `hide-env`
/// * Misc: `ignore`, `collect`, `export` (generic keyword), `attr *`
fn create_xeno_lang_context() -> EngineState {
	let mut engine_state = EngineState::new();
	let delta = {
		let mut working_set = StateWorkingSet::new(&engine_state);
		macro_rules! bind {
			( $( $cmd:expr ),* $(,)? ) => {
				$( working_set.add_decl(Box::new($cmd)); )*
			};
		}
		bind! {
			// Definitions and modules
			xeno_nu_cmd_lang::Def,
			xeno_nu_cmd_lang::ExportDef,
			xeno_nu_cmd_lang::Module,
			xeno_nu_cmd_lang::ExportModule,
			xeno_nu_cmd_lang::Use,
			xeno_nu_cmd_lang::ExportUse,
			// Bindings
			xeno_nu_cmd_lang::Let,
			xeno_nu_cmd_lang::Mut,
			xeno_nu_cmd_lang::Const,
			xeno_nu_cmd_lang::ExportConst,
			// Control flow
			xeno_nu_cmd_lang::If,
			xeno_nu_cmd_lang::Match,
			xeno_nu_cmd_lang::Do,
			xeno_nu_cmd_lang::Try,
			xeno_nu_cmd_lang::Return,
			// Output and error handling
			xeno_nu_cmd_lang::Echo,
			xeno_nu_cmd_lang::Error,
			xeno_nu_cmd_lang::ErrorMake,
		}
		working_set.render()
	};
	engine_state.merge_delta(delta).expect("merge xeno lang context");
	engine_state
}

/// Result of parsing and validating a Nu script, including the set of
/// declarations introduced by the script (as opposed to engine builtins).
#[derive(Debug)]
pub struct ParseResult {
	pub block: Arc<Block>,
	pub script_decl_ids: Vec<DeclId>,
}

/// Controls what top-level constructs are allowed in a parsed script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsePolicy {
	/// Allow any expression at top level (used by `config.nu`).
	Script,
	/// Only declarations, imports, constants, and modules at top
	/// level (used by `xeno.nu`). Top-level `let`/`mut`/expressions are
	/// rejected.
	ModuleOnly,
}

/// Parses Nu source, validates the sandbox, and merges into the engine state.
///
/// Equivalent to [`parse_and_validate_with_policy`] with [`ParsePolicy::Script`].
pub fn parse_and_validate(engine_state: &mut EngineState, fname: &str, source: &str, config_root: Option<&Path>) -> Result<ParseResult, String> {
	parse_and_validate_with_policy(engine_state, fname, source, config_root, ParsePolicy::Script)
}

/// Parses Nu source, validates the sandbox and parse policy, and merges into
/// the engine state.
///
/// Returns the parsed block and the set of declaration IDs introduced by this
/// script (i.e., `def`/`export def` in the source and its modules). Builtins
/// from `nu-cmd-lang` are excluded.
pub fn parse_and_validate_with_policy(
	engine_state: &mut EngineState,
	fname: &str,
	source: &str,
	config_root: Option<&Path>,
	policy: ParsePolicy,
) -> Result<ParseResult, String> {
	let mut working_set = StateWorkingSet::new(engine_state);
	let base_decls = working_set.permanent_state.num_decls();

	let block = xeno_nu_parser::parse(&mut working_set, Some(fname), source.as_bytes(), false);

	if let Some(error) = working_set.parse_errors.first() {
		return Err(format!("Nu parse error: {error}"));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(format!("Nu compile error: {error}"));
	}

	ensure_sandboxed(&working_set, block.as_ref(), config_root)?;

	if policy == ParsePolicy::ModuleOnly {
		ensure_module_only(&working_set, block.as_ref())?;
	}

	let added_decls = working_set.delta.num_decls();
	let script_decl_ids: Vec<DeclId> = (0..added_decls).map(|i| DeclId::new(base_decls + i)).collect();

	if policy == ParsePolicy::ModuleOnly {
		check_reserved_names(&working_set, &script_decl_ids)?;
	}

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;

	Ok(ParseResult { block, script_decl_ids })
}

/// Built-in command names that user scripts must not shadow.
const RESERVED_COMMAND_NAMES: &[&str] = &["action", "command", "editor", "nu run", "xeno ctx"];

/// Reject user declarations that shadow built-in xeno commands.
fn check_reserved_names(working_set: &xeno_nu_protocol::engine::StateWorkingSet<'_>, script_decl_ids: &[DeclId]) -> Result<(), String> {
	for &decl_id in script_decl_ids {
		let name = working_set.get_decl(decl_id).name();
		if RESERVED_COMMAND_NAMES.contains(&name) {
			return Err(format!("Nu script error: '{name}' is a reserved Xeno built-in command; rename your definition"));
		}
	}
	Ok(())
}

/// Declaration names allowed at top level under [`ParsePolicy::ModuleOnly`].
const MODULE_ONLY_ALLOWED_DECLS: &[&str] = &[
	"export def",
	"def",
	"export use",
	"use",
	"export const",
	"const",
	"export module",
	"module",
];

/// Validates that a block contains only declarations at top level.
fn ensure_module_only(working_set: &StateWorkingSet<'_>, block: &Block) -> Result<(), String> {
	for pipeline in &block.pipelines {
		for element in &pipeline.elements {
			if element.redirection.is_some() {
				return Err("module-only script: top-level redirections are not allowed".to_string());
			}
			match &element.expr.expr {
				Expr::Call(call) => {
					let decl_name = working_set.get_decl(call.decl_id).name();
					if !MODULE_ONLY_ALLOWED_DECLS.contains(&decl_name) {
						return Err(format!(
							"module-only script: top-level '{decl_name}' is not allowed; only def/use/const/module are permitted"
						));
					}
				}
				Expr::Nothing => {}
				other => {
					return Err(format!(
						"module-only script: top-level expressions are not allowed; only def/use/const/module are permitted (found {:?})",
						std::mem::discriminant(other)
					));
				}
			}
		}
	}
	Ok(())
}

/// Evaluates a parsed block and returns the resulting value.
pub fn evaluate_block(engine_state: &EngineState, block: &Block) -> Result<Value, String> {
	let mut stack = Stack::new();
	let eval_block = xeno_nu_engine::get_eval_block(engine_state);
	let execution = eval_block(engine_state, &mut stack, block, PipelineData::empty()).map_err(|error| format!("Nu runtime error: {error}"))?;
	execution.body.into_value(Span::unknown()).map_err(|error| format!("Nu runtime error: {error}"))
}

/// Calls an already-registered function by declaration ID with string positional
/// arguments and optional environment variables injected via the stack.
///
/// Uses `eval_call` directly — no source parsing, no engine state mutation, no
/// delta merge. The engine state is borrowed immutably; per-call env lives on
/// the stack and is discarded after evaluation.
pub fn call_function(engine_state: &EngineState, decl_id: DeclId, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
	let span = Span::unknown();
	let mut call = resolve_decl_call(engine_state, decl_id, span)?;
	for arg in args {
		call.add_positional(Expression::new_unknown(Expr::String(arg.clone()), span, Type::String));
	}

	let mut stack = Stack::new();
	for (key, value) in env {
		stack.add_env_var((*key).to_string(), value.clone());
	}

	let result = xeno_nu_engine::eval_call::<WithoutDebug>(engine_state, &mut stack, &call, PipelineData::empty())
		.map_err(|error| format!("Nu runtime error: {error}"))?;
	result.into_value(span).map_err(|error| format!("Nu runtime error: {error}"))
}

/// Like [`call_function`] but consumes owned args and env to avoid cloning on
/// the hot path. Use this from persistent worker threads where the data is
/// already owned.
pub fn call_function_owned(engine_state: &EngineState, decl_id: DeclId, args: Vec<String>, env: Vec<(String, Value)>) -> Result<Value, String> {
	let span = Span::unknown();
	let mut call = resolve_decl_call(engine_state, decl_id, span)?;
	for arg in args {
		call.add_positional(Expression::new_unknown(Expr::String(arg), span, Type::String));
	}

	let mut stack = Stack::new();
	for (key, value) in env {
		stack.add_env_var(key, value);
	}

	let result = xeno_nu_engine::eval_call::<WithoutDebug>(engine_state, &mut stack, &call, PipelineData::empty())
		.map_err(|error| format!("Nu runtime error: {error}"))?;
	result.into_value(span).map_err(|error| format!("Nu runtime error: {error}"))
}

fn resolve_decl_call(_engine_state: &EngineState, decl_id: DeclId, span: Span) -> Result<Call, String> {
	let mut call = Call::new(span);
	call.decl_id = decl_id;
	Ok(call)
}

/// Looks up a declaration by name in the engine state.
pub fn find_decl(engine_state: &EngineState, name: &str) -> Option<DeclId> {
	engine_state.find_decl(name.as_bytes(), &[])
}

#[cfg(test)]
mod tests;
