use std::path::Path;

use super::*;

fn write_script(dir: &Path, source: &str) {
	std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
}

#[test]
fn runtime_load_resolve_and_call() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { 42 }");

	let runtime = NuProgram::compile_macro_from_dir(temp.path()).expect("runtime should load");
	let function = runtime.resolve_export("go").expect("go should resolve");

	let value = runtime.call_export(function, &[], &[]).expect("call should succeed");
	assert_eq!(value.as_int().expect("value should be int"), 42);
}

#[test]
fn runtime_resolve_rejects_builtins() {
	let temp = tempfile::tempdir().expect("temp dir should exist");
	write_script(temp.path(), "export def go [] { 42 }");

	let runtime = NuProgram::compile_macro_from_dir(temp.path()).expect("runtime should load");
	assert!(runtime.resolve_export("go").is_some());
	assert!(runtime.resolve_export("if").is_none());
}

#[test]
fn eval_source_returns_parse_error_for_invalid_parse() {
	let err = NuProgram::compile_config_script("config.nu", "^echo hi", None).expect_err("external call should be parse failure");
	assert!(matches!(err, CompileError::Parse(_)));
}

#[test]
fn eval_source_returns_runtime_error_for_eval_failure() {
	let program = NuProgram::compile_config_script("config.nu", "error make { msg: 'boom' }", None).expect("script should compile");
	let err = program.execute_root().expect_err("error command should fail at runtime");
	assert!(matches!(err, ExecError::Runtime(_)));
}

#[test]
fn load_rejects_oversized_script_file() {
	let temp = tempfile::tempdir().expect("temp dir");
	let big = "x".repeat(MAX_SCRIPT_BYTES + 1);
	std::fs::write(temp.path().join("xeno.nu"), &big).unwrap();
	let err = NuProgram::compile_macro_from_dir(temp.path()).expect_err("oversized script file should be rejected");
	assert!(err.to_string().contains("exceeds"), "got: {err}");
}

#[test]
fn load_source_rejects_oversized_source() {
	let temp = tempfile::tempdir().expect("temp dir");
	let big = "x".repeat(MAX_SCRIPT_BYTES + 1);
	let err = NuProgram::compile_macro_source(temp.path(), &temp.path().join("xeno.nu"), &big).expect_err("oversized source should be rejected");
	assert!(err.to_string().contains("exceeds"), "got: {err}");
}

#[test]
fn eval_source_rejects_oversized_source() {
	let big = "x".repeat(MAX_SCRIPT_BYTES + 1);
	let err = NuProgram::compile_config_script("config.nu", &big, None).expect_err("oversized eval source should be rejected");
	assert!(matches!(err, CompileError::Parse(_)));
}

#[test]
fn resolve_export_rejects_private_defs() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "def hidden [] { 1 }\nexport def visible [] { hidden }");

	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	assert!(program.resolve_export("visible").is_some(), "exported def should resolve");
	assert!(program.resolve_export("hidden").is_none(), "private def must not resolve");
}

#[test]
fn checked_decl_id_rejects_forged_export_id() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "def hidden [] { 1 }\nexport def visible [] { hidden }");

	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");

	// Find hidden's raw DeclId by looking at script_decls minus export_decls.
	let hidden_id = program
		.script_decls
		.iter()
		.find(|id| !program.export_decls.contains(id))
		.expect("hidden decl should exist in script_decls");

	let forged = ExportId::from_raw(hidden_id.get());
	let err = program.call_export(forged, &[], &[]).expect_err("forged ExportId should fail");
	assert!(matches!(err, ExecError::InvalidExportId(_)));
}

#[test]
fn exports_returns_only_exported_names() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "def hidden [] { 1 }\nexport def alpha [] { 2 }\nexport def beta [] { 3 }");

	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let exports = program.exports();
	let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
	assert_eq!(names, vec!["alpha", "beta"], "exports should be sorted and contain only exported defs");
}
