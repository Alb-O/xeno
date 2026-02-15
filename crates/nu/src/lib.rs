//! Shared Nu script sandbox and evaluation helpers for Xeno.
//!
//! Provides a sandboxed Nu evaluation environment used by both the config
//! system (`config.nu`) and the editor macro runtime (`xeno.nu`).
//!
//! Sandboxing is composed from:
//! * A minimal `nu-cmd-lang` engine context (no filesystem/network/plugin
//!   command sets loaded).
//! * AST-level policy checks for external execution, redirection/glob usage,
//!   overlay/source loading, and looping.
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

mod sandbox;

use std::path::Path;
use std::sync::Arc;

use nu_protocol::ast::{Block, Call, Expr, Expression};
use nu_protocol::config::Config;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{DeclId, PipelineData, Span, Type, Value};
pub use sandbox::ensure_sandboxed;

const XENO_NU_RECURSION_LIMIT: i64 = 64;

/// Creates a minimal Nu engine state suitable for sandboxed evaluation.
///
/// Includes core language commands (`def`, `if`, `let`, `use`, etc.) but no
/// filesystem, network, or external-command support. Sets `PWD` to the
/// canonicalized `config_root` if provided.
pub fn create_engine_state(config_root: Option<&Path>) -> EngineState {
	let mut engine_state = nu_cmd_lang::create_default_context();
	let mut config: Config = engine_state.get_config().as_ref().clone();
	config.recursion_limit = XENO_NU_RECURSION_LIMIT;
	engine_state.set_config(config);

	if let Some(cwd) = config_root.and_then(|p| std::fs::canonicalize(p).ok()) {
		engine_state.add_env_var("PWD".to_string(), Value::string(cwd.to_string_lossy().to_string(), Span::unknown()));
	}
	engine_state
}

/// Result of parsing and validating a Nu script, including the set of
/// declarations introduced by the script (as opposed to engine builtins).
pub struct ParseResult {
	pub block: Arc<Block>,
	pub script_decl_ids: Vec<DeclId>,
}

/// Parses Nu source, validates the sandbox, and merges into the engine state.
///
/// Returns the parsed block and the set of declaration IDs introduced by this
/// script (i.e., `def`/`export def` in the source and its modules). Builtins
/// from `nu-cmd-lang` are excluded.
pub fn parse_and_validate(engine_state: &mut EngineState, fname: &str, source: &str, config_root: Option<&Path>) -> Result<ParseResult, String> {
	let mut working_set = StateWorkingSet::new(engine_state);
	let base_decls = working_set.permanent_state.num_decls();

	let block = nu_parser::parse(&mut working_set, Some(fname), source.as_bytes(), false);

	if let Some(error) = working_set.parse_errors.first() {
		return Err(format!("Nu parse error: {error}"));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(format!("Nu compile error: {error}"));
	}

	ensure_sandboxed(&working_set, block.as_ref(), config_root)?;

	let added_decls = working_set.delta.num_decls();
	let script_decl_ids: Vec<DeclId> = (0..added_decls).map(|i| DeclId::new(base_decls + i)).collect();

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;

	Ok(ParseResult { block, script_decl_ids })
}

/// Evaluates a parsed block and returns the resulting value.
pub fn evaluate_block(engine_state: &EngineState, block: &Block) -> Result<Value, String> {
	let mut stack = Stack::new();
	let eval_block = nu_engine::get_eval_block(engine_state);
	let execution = eval_block(engine_state, &mut stack, block, PipelineData::empty()).map_err(|error| format!("Nu runtime error: {error}"))?;
	execution.body.into_value(Span::unknown()).map_err(|error| format!("Nu runtime error: {error}"))
}

/// Calls an already-registered function by declaration ID with string positional
/// arguments and optional environment variables injected via the stack.
///
/// Uses `eval_call` directly — no source parsing, no engine state mutation, no
/// delta merge. Alias declarations are unwrapped to their underlying internal
/// call so alias-backed entrypoints (e.g. `export alias go = ...`) are
/// executable through this API. The engine state is borrowed immutably;
/// per-call env lives on the stack and is discarded after evaluation.
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

	let result =
		nu_engine::eval_call::<WithoutDebug>(engine_state, &mut stack, &call, PipelineData::empty()).map_err(|error| format!("Nu runtime error: {error}"))?;
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

	let result =
		nu_engine::eval_call::<WithoutDebug>(engine_state, &mut stack, &call, PipelineData::empty()).map_err(|error| format!("Nu runtime error: {error}"))?;
	result.into_value(span).map_err(|error| format!("Nu runtime error: {error}"))
}

fn resolve_decl_call(engine_state: &EngineState, decl_id: DeclId, span: Span) -> Result<Call, String> {
	let decl = engine_state.get_decl(decl_id);
	if let Some(alias) = decl.as_alias() {
		match &alias.wrapped_call.expr {
			Expr::Call(wrapped_call) => Ok((**wrapped_call).clone()),
			_ => Err(format!(
				"Nu runtime error: alias '{}' expands to an external command, which is disabled",
				alias.name
			)),
		}
	} else {
		let mut call = Call::new(span);
		call.decl_id = decl_id;
		Ok(call)
	}
}

/// Looks up a declaration by name in the engine state.
pub fn find_decl(engine_state: &EngineState, name: &str) -> Option<DeclId> {
	engine_state.find_decl(name.as_bytes(), &[])
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn call_function_with_args_and_env() {
		let mut engine_state = create_engine_state(None);
		let source = "export def greet [name: string] { $\"hello ($name) ($env.XENO_CTX)\" }";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

		let decl_id = find_decl(&engine_state, "greet").expect("greet should be registered");
		assert!(parsed.script_decl_ids.contains(&decl_id), "greet should be in script_decl_ids");
		let ctx_val = Value::string("ctx-value", Span::unknown());
		let result = call_function(&engine_state, decl_id, &["world".to_string()], &[("XENO_CTX", ctx_val)]).expect("call should succeed");
		assert_eq!(result.as_str().unwrap(), "hello world ctx-value");
	}

	#[test]
	fn call_function_does_not_mutate_engine_state() {
		let mut engine_state = create_engine_state(None);
		let source = "export def echo-it [x: string] { $x }";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let _ = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");

		let num_blocks_before = engine_state.num_blocks();
		let decl_id = find_decl(&engine_state, "echo-it").expect("echo-it should be registered");

		for _ in 0..10 {
			let _ = call_function(&engine_state, decl_id, &["hi".to_string()], &[]).expect("call should succeed");
		}

		assert_eq!(engine_state.num_blocks(), num_blocks_before, "engine state should not accumulate blocks");
	}

	#[test]
	fn script_decl_ids_excludes_builtins() {
		let mut engine_state = create_engine_state(None);
		let source = "export def my-func [] { 42 }";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

		// "if" is a builtin — it should not be in script_decl_ids
		let if_decl = find_decl(&engine_state, "if").expect("if should exist");
		assert!(!parsed.script_decl_ids.contains(&if_decl), "builtins must not appear in script_decl_ids");

		// "my-func" should be in script_decl_ids
		let my_func = find_decl(&engine_state, "my-func").expect("my-func should exist");
		assert!(parsed.script_decl_ids.contains(&my_func), "script defs must appear in script_decl_ids");
	}

	#[test]
	fn parse_and_validate_registers_defs_without_eval() {
		let mut engine_state = create_engine_state(None);
		let source = "export def go [] { 1 }";
		let _parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		// No evaluate_block — defs should still be registered by parse+merge.
		assert!(find_decl(&engine_state, "go").is_some(), "go should be registered without evaluation");
	}

	#[test]
	fn call_function_supports_alias_decls() {
		let mut engine_state = create_engine_state(None);
		let source = "export alias go = echo editor:stats";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

		let decl_id = find_decl(&engine_state, "go").expect("go should be registered");
		assert!(parsed.script_decl_ids.contains(&decl_id), "go alias should be in script_decl_ids");

		let result = call_function(&engine_state, decl_id, &[], &[]).expect("alias call should succeed");
		assert_eq!(result.as_str().unwrap(), "editor:stats");
	}

	#[test]
	fn call_function_owned_supports_alias_decls() {
		let mut engine_state = create_engine_state(None);
		let source = "export alias go = echo editor:stats";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

		let decl_id = find_decl(&engine_state, "go").expect("go should be registered");
		assert!(parsed.script_decl_ids.contains(&decl_id), "go alias should be in script_decl_ids");

		let result = call_function_owned(&engine_state, decl_id, vec![], vec![]).expect("alias call should succeed");
		assert_eq!(result.as_str().unwrap(), "editor:stats");
	}

	#[test]
	fn recursive_function_hits_recursion_limit() {
		let mut engine_state = create_engine_state(None);
		let source = "export def recur [] { recur }\nrecur";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("recursive script should parse");
		let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("recursive script must error");
		let msg = err.to_ascii_lowercase();
		assert!(msg.contains("recursion") || msg.contains("stack") || msg.contains("overflow"), "{err}");
	}

	#[test]
	fn str_shim_defs_work_in_sandbox() {
		let mut engine_state = create_engine_state(None);
		let source = r#"export def "str ends-with" [suffix: string] { $in ends-with $suffix }
"abc" | str ends-with "bc""#;
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("str shim should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("str shim should evaluate");
		assert_eq!(value, nu_protocol::Value::test_bool(true));
	}
}
