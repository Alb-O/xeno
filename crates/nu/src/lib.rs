//! Shared Nu script sandbox and evaluation helpers for Xeno.
//!
//! Provides a sandboxed Nu evaluation environment used by both the config
//! system (`config.nu`) and the editor macro runtime (`xeno.nu`). The sandbox
//! enforces an AST-level policy that blocks external commands, filesystem I/O,
//! networking, looping, and module paths that escape the config root.

mod sandbox;

use std::path::Path;
use std::sync::Arc;

use nu_protocol::ast::Block;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{PipelineData, Span, Value};

pub use sandbox::ensure_sandboxed;

/// Creates a minimal Nu engine state suitable for sandboxed evaluation.
///
/// Includes core language commands (`def`, `if`, `let`, `use`, etc.) but no
/// filesystem, network, or external-command support. Sets `PWD` to the
/// canonicalized `config_root` if provided.
pub fn create_engine_state(config_root: Option<&Path>) -> EngineState {
	let mut engine_state = nu_cmd_lang::create_default_context();
	if let Some(cwd) = config_root.and_then(|p| std::fs::canonicalize(p).ok()) {
		engine_state.add_env_var(
			"PWD".to_string(),
			Value::string(cwd.to_string_lossy().to_string(), Span::unknown()),
		);
	}
	engine_state
}

/// Parses Nu source, validates the sandbox, and merges into the engine state.
///
/// Returns the parsed block on success, or a human-readable error string.
pub fn parse_and_validate(
	engine_state: &mut EngineState,
	fname: &str,
	source: &str,
	config_root: Option<&Path>,
) -> Result<Arc<Block>, String> {
	let mut working_set = StateWorkingSet::new(engine_state);
	let block = nu_parser::parse(&mut working_set, Some(fname), source.as_bytes(), false);

	if let Some(error) = working_set.parse_errors.first() {
		return Err(format!("Nu parse error: {error}"));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(format!("Nu compile error: {error}"));
	}

	ensure_sandboxed(&working_set, block.as_ref(), config_root)?;

	let delta = working_set.render();
	engine_state
		.merge_delta(delta)
		.map_err(|error| format!("Nu merge error: {error}"))?;

	Ok(block)
}

/// Evaluates a parsed block and returns the resulting value.
pub fn evaluate_block(engine_state: &EngineState, block: &Block) -> Result<Value, String> {
	let mut stack = Stack::new();
	let eval_block = nu_engine::get_eval_block(engine_state);
	let execution = eval_block(engine_state, &mut stack, block, PipelineData::empty())
		.map_err(|error| format!("Nu runtime error: {error}"))?;
	execution
		.body
		.into_value(Span::unknown())
		.map_err(|error| format!("Nu runtime error: {error}"))
}

/// Escapes a string for safe inclusion in a Nu source snippet.
pub fn quote_nu_string(input: &str) -> String {
	let mut out = String::with_capacity(input.len() + 2);
	out.push('"');
	for ch in input.chars() {
		match ch {
			'\\' => out.push_str("\\\\"),
			'"' => out.push_str("\\\""),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			_ => out.push(ch),
		}
	}
	out.push('"');
	out
}

/// Builds a Nu call source string from a function name and arguments.
pub fn build_call_source(fn_name: &str, args: &[String]) -> Result<String, String> {
	if fn_name.is_empty() {
		return Err("function name cannot be empty".to_string());
	}
	if !fn_name
		.chars()
		.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
	{
		return Err("function name contains unsupported characters".to_string());
	}

	let mut src = fn_name.to_string();
	for arg in args {
		src.push(' ');
		src.push_str(&quote_nu_string(arg));
	}
	Ok(src)
}
