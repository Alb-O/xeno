use std::path::Path;

use super::*;

fn write_script(dir: &Path, source: &str) {
	std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
}

#[test]
fn runtime_load_resolve_and_call() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { 42 }");

	let runtime = Runtime::load(temp.path()).expect("runtime should load");
	let function = runtime.resolve_function("go").expect("go should resolve");

	let value = runtime.call(function, &[], &[]).expect("call should succeed");
	assert_eq!(value.as_int().expect("value should be int"), 42);
}

#[test]
fn runtime_resolve_rejects_builtins() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { 42 }");

	let runtime = Runtime::load(temp.path()).expect("runtime should load");
	assert!(runtime.resolve_function("go").is_some());
	assert!(runtime.resolve_function("if").is_none());
}

#[test]
fn eval_source_returns_parse_error_for_invalid_parse() {
	let err = eval_source("config.nu", "^echo hi", None).expect_err("external call should be parse failure");
	assert!(matches!(err, EvalError::Parse(_)));
}

#[test]
fn eval_source_returns_runtime_error_for_eval_failure() {
	let err = eval_source("config.nu", "error make { msg: 'boom' }", None).expect_err("error command should fail at runtime");
	assert!(matches!(err, EvalError::Runtime(_)));
}
