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

mod sandbox;

use std::path::Path;
use std::sync::Arc;

use nu_protocol::ast::{Block, Call, Expr, Expression};
use nu_protocol::config::Config;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{DeclId, PipelineData, Span, Type, Value};
pub use sandbox::ensure_sandboxed;

/// Built-in xeno prelude module source, loaded into every engine state.
///
/// Provides invocation constructors and string helpers so user scripts can
/// use these directly (they are exported into the default scope).
/// Users can also `use xeno *` explicitly in their scripts.
/// Prelude version — bump when prelude API changes.
pub const XENO_PRELUDE_VERSION: i64 = 1;

const XENO_PRELUDE_SOURCE: &str = r#"
module xeno {
    # Invocation constructors — return structured records for decode.
    # Optional fields use null (decode treats null as absent).
    export def action [name: string, --count: int = 1, --extend, --register: string, --char: string] {
        { kind: "action", name: $name, count: $count, extend: $extend, register: $register, char: $char }
    }
    export def command [name: string, ...args: string] { { kind: "command", name: $name, args: $args } }
    export def editor [name: string, ...args: string] { { kind: "editor", name: $name, args: $args } }
    export def "nu run" [name: string, ...args: string] { { kind: "nu", name: $name, args: $args } }
    # Std-like small utilities (pure language; no nu-command deps).
    export def default [value] { if $in == null { $value } else { $in } }
    export def is-null [] { $in == null }
    # String helpers using Nu operators (no command dependencies).
    export def "str ends-with" [suffix: string] { $in ends-with $suffix }
    export def "str starts-with" [prefix: string] { $in starts-with $prefix }
    export def "str contains" [needle: string] { $in like $needle }
    export const XENO_PRELUDE_VERSION = 1
}
use xeno *
"#;

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

	load_xeno_prelude(&mut engine_state)?;
	Ok(engine_state)
}

/// Parses and merges the built-in xeno prelude into the engine state.
///
/// The prelude is loaded as a virtual `<xeno/mod.nu>` file so `use xeno *`
/// works in user scripts. Returns an error on parse/compile failure.
fn load_xeno_prelude(engine_state: &mut EngineState) -> Result<(), String> {
	let mut working_set = StateWorkingSet::new(engine_state);
	let _block = nu_parser::parse(&mut working_set, Some("<xeno/prelude>"), XENO_PRELUDE_SOURCE.as_bytes(), false);

	if let Some(error) = working_set.parse_errors.first() {
		return Err(format!("xeno prelude parse error: {error}"));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(format!("xeno prelude compile error: {error}"));
	}

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|e| format!("xeno prelude merge error: {e}"))?;
	Ok(())
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
			nu_cmd_lang::Def,
			nu_cmd_lang::ExportDef,
			nu_cmd_lang::Module,
			nu_cmd_lang::ExportModule,
			nu_cmd_lang::Use,
			nu_cmd_lang::ExportUse,
			nu_cmd_lang::Alias,
			nu_cmd_lang::ExportAlias,
			// Bindings
			nu_cmd_lang::Let,
			nu_cmd_lang::Mut,
			nu_cmd_lang::Const,
			nu_cmd_lang::ExportConst,
			// Control flow
			nu_cmd_lang::If,
			nu_cmd_lang::Match,
			nu_cmd_lang::Do,
			nu_cmd_lang::Try,
			nu_cmd_lang::Return,
			// Output and error handling
			nu_cmd_lang::Echo,
			nu_cmd_lang::Error,
			nu_cmd_lang::ErrorMake,
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
	/// Only declarations, imports, constants, aliases, and modules at top
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

	let block = nu_parser::parse(&mut working_set, Some(fname), source.as_bytes(), false);

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

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|error| format!("Nu merge error: {error}"))?;

	Ok(ParseResult { block, script_decl_ids })
}

/// Declaration names allowed at top level under [`ParsePolicy::ModuleOnly`].
const MODULE_ONLY_ALLOWED_DECLS: &[&str] = &[
	"export def",
	"def",
	"export use",
	"use",
	"export const",
	"const",
	"export alias",
	"alias",
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
					if !MODULE_ONLY_ALLOWED_DECLS.iter().any(|allowed| *allowed == decl_name) {
						return Err(format!(
							"module-only script: top-level '{decl_name}' is not allowed; only def/use/const/alias/module are permitted"
						));
					}
				}
				Expr::Nothing => {}
				other => {
					return Err(format!(
						"module-only script: top-level expressions are not allowed; only def/use/const/alias/module are permitted (found {:?})",
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
		let mut engine_state = create_engine_state(None).expect("engine state");
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
		let mut engine_state = create_engine_state(None).expect("engine state");
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
		let mut engine_state = create_engine_state(None).expect("engine state");
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
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "export def go [] { 1 }";
		let _parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		// No evaluate_block — defs should still be registered by parse+merge.
		assert!(find_decl(&engine_state, "go").is_some(), "go should be registered without evaluation");
	}

	#[test]
	fn call_function_supports_alias_decls() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "export alias go = echo editor:stats";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

		let decl_id = find_decl(&engine_state, "go").expect("go should be registered");
		assert!(parsed.script_decl_ids.contains(&decl_id), "go alias should be in script_decl_ids");

		let result = call_function(&engine_state, decl_id, &[], &[]).expect("alias call should succeed");
		assert_eq!(result.as_str().unwrap(), "editor:stats");
	}

	#[test]
	fn call_function_owned_supports_alias_decls() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "export alias go = echo editor:stats";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");

		let decl_id = find_decl(&engine_state, "go").expect("go should be registered");
		assert!(parsed.script_decl_ids.contains(&decl_id), "go alias should be in script_decl_ids");

		let result = call_function_owned(&engine_state, decl_id, vec![], vec![]).expect("alias call should succeed");
		assert_eq!(result.as_str().unwrap(), "editor:stats");
	}

	#[test]
	fn recursive_function_hits_recursion_limit() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "export def recur [] { recur }\nrecur";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("recursive script should parse");
		let err = evaluate_block(&engine_state, parsed.block.as_ref()).expect_err("recursive script must error");
		let msg = err.to_ascii_lowercase();
		assert!(msg.contains("recursion") || msg.contains("stack") || msg.contains("overflow"), "{err}");
	}

	#[test]
	fn module_only_accepts_export_def() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "export def go [] { 1 }";
		parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly).expect("export def should be allowed");
	}

	#[test]
	fn module_only_accepts_use_and_const() {
		let temp = tempfile::tempdir().expect("temp dir");
		std::fs::write(temp.path().join("helper.nu"), "export def x [] { 1 }").unwrap();
		let mut engine_state = create_engine_state(Some(temp.path())).expect("engine state");
		let source = "const A = 42\nuse helper.nu *\nexport def go [] { x }";
		parse_and_validate_with_policy(&mut engine_state, "<test>", source, Some(temp.path()), ParsePolicy::ModuleOnly)
			.expect("const + use + export def should be allowed");
	}

	#[test]
	fn module_only_rejects_expression() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let err =
			parse_and_validate_with_policy(&mut engine_state, "<test>", "42", None, ParsePolicy::ModuleOnly).expect_err("bare expression should be rejected");
		assert!(err.contains("module-only"), "{err}");
	}

	#[test]
	fn module_only_rejects_let() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "let x = 1", None, ParsePolicy::ModuleOnly)
			.expect_err("let should be rejected in module-only");
		assert!(err.contains("module-only") && err.contains("let"), "{err}");
	}

	#[test]
	fn module_only_rejects_mut() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let err = parse_and_validate_with_policy(&mut engine_state, "<test>", "mut x = 1", None, ParsePolicy::ModuleOnly)
			.expect_err("mut should be rejected in module-only");
		assert!(err.contains("module-only") && err.contains("mut"), "{err}");
	}

	#[test]
	fn script_policy_allows_expressions() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		parse_and_validate_with_policy(&mut engine_state, "<test>", "42", None, ParsePolicy::Script)
			.expect("bare expression should be allowed in Script policy");
	}

	#[test]
	fn prelude_action_constructor_returns_record() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "action move_right --count 2";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		let record = value.as_record().expect("should be record");
		assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "action");
		assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
		assert_eq!(record.get("count").unwrap().as_int().unwrap(), 2);
		assert!(record.get("register").unwrap().is_nothing());
		assert!(record.get("char").unwrap().is_nothing());
	}

	#[test]
	fn prelude_command_constructor_returns_record() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "command write foo.txt";
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		let record = value.as_record().expect("should be record");
		assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "command");
		assert_eq!(record.get("name").unwrap().as_str().unwrap(), "write");
		let args = record.get("args").unwrap().as_list().unwrap();
		assert_eq!(args.len(), 1);
		assert_eq!(args[0].as_str().unwrap(), "foo.txt");
	}

	#[test]
	fn prelude_str_helpers_work() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = r#""hello world" | str ends-with "world""#;
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		assert_eq!(value, Value::test_bool(true));
	}

	#[test]
	fn prelude_available_in_module_only_scripts() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = "export def go [] { action move_right --count 5 }";
		let parsed = parse_and_validate_with_policy(&mut engine_state, "<test>", source, None, ParsePolicy::ModuleOnly)
			.expect("prelude should be available in module-only scripts");
		let decl_id = find_decl(&engine_state, "go").expect("go should exist");
		let value = call_function(&engine_state, decl_id, &[], &[]).expect("should call");
		let record = value.as_record().expect("should be record");
		assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
		assert_eq!(record.get("count").unwrap().as_int().unwrap(), 5);
	}

	#[test]
	fn str_shim_defs_work_in_sandbox() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = r#"export def "str ends-with" [suffix: string] { $in ends-with $suffix }
"abc" | str ends-with "bc""#;
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("str shim should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("str shim should evaluate");
		assert_eq!(value, nu_protocol::Value::test_bool(true));
	}

	#[test]
	fn prelude_default_replaces_null() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = r#"null | default "x""#;
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		assert_eq!(value, nu_protocol::Value::test_string("x"));
	}

	#[test]
	fn prelude_default_preserves_non_null() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let source = r#""y" | default "x""#;
		let parsed = parse_and_validate(&mut engine_state, "<test>", source, None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		assert_eq!(value, nu_protocol::Value::test_string("y"));
	}

	#[test]
	fn prelude_is_null() {
		let mut engine_state = create_engine_state(None).expect("engine state");
		let parsed = parse_and_validate(&mut engine_state, "<test>", "null | is-null", None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		assert_eq!(value, nu_protocol::Value::test_bool(true));

		let mut engine_state = create_engine_state(None).expect("engine state");
		let parsed = parse_and_validate(&mut engine_state, "<test>", r#""hi" | is-null"#, None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		assert_eq!(value, nu_protocol::Value::test_bool(false));
	}

	#[test]
	fn create_engine_state_succeeds_and_exposes_prelude_version() {
		let mut engine_state = create_engine_state(None).expect("engine state should be created");
		let parsed = parse_and_validate(&mut engine_state, "<test>", "$XENO_PRELUDE_VERSION", None).expect("should parse");
		let value = evaluate_block(&engine_state, parsed.block.as_ref()).expect("should evaluate");
		assert_eq!(value, nu_protocol::Value::test_int(XENO_PRELUDE_VERSION));
	}
}
