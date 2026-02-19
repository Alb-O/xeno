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

	let value = runtime.call_export(function, &[], &[], None).expect("call should succeed");
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

	// Forge an ExportId with a raw value that is definitely not in the export set.
	// Use 999999 which is far beyond any real DeclId.
	let forged = ExportId::from_raw(999999);
	let err = program.call_export(forged, &[], &[], None).expect_err("forged ExportId should fail");
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

#[test]
fn module_export_use_explicit() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "module foo { export def bar [] { 1 } }\nexport use foo bar");

	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	assert!(program.resolve_export("bar").is_some(), "re-exported bar should resolve");
	let exports = program.exports();
	let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
	assert_eq!(names, vec!["bar"]);
}

#[test]
fn module_export_use_star() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "module foo { export def a [] { 1 }; export def b [] { 2 } }\nexport use foo *");

	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let exports = program.exports();
	let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
	assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn module_private_not_exported() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "module foo { export def public [] { 1 }; def private [] { 2 } }\nexport use foo *");

	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	assert!(program.resolve_export("public").is_some());
	assert!(program.resolve_export("private").is_none(), "private def inside module must not be exported");
}

// --- Step 6: Call input validation tests (at NuProgram API level) ---

fn varargs_program() -> (NuProgram, ExportId) {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "export def accept [...args] { $args | length }");
	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let export = program.resolve_export("accept").expect("accept should resolve");
	// Leak tempdir so path stays valid for program's lifetime
	std::mem::forget(temp);
	(program, export)
}

#[test]
fn call_at_max_args_succeeds() {
	use xeno_invocation::nu::DEFAULT_CALL_LIMITS;
	let (program, export) = varargs_program();
	let args: Vec<String> = (0..DEFAULT_CALL_LIMITS.max_args).map(|i| i.to_string()).collect();
	program.call_export(export, &args, &[], None).expect("max_args should succeed");
}

#[test]
fn call_over_max_args_rejected() {
	use xeno_invocation::nu::DEFAULT_CALL_LIMITS;
	let (program, export) = varargs_program();
	let args: Vec<String> = (0..DEFAULT_CALL_LIMITS.max_args + 1).map(|i| i.to_string()).collect();
	let err = program.call_export(export, &args, &[], None).expect_err("over max_args should be rejected");
	assert!(matches!(err, ExecError::CallValidation(CallValidationError::ArgsTooMany { .. })), "got: {err}");
}

#[test]
fn call_oversize_arg_rejected() {
	use xeno_invocation::nu::DEFAULT_CALL_LIMITS;
	let (program, export) = varargs_program();
	let big_arg = "x".repeat(DEFAULT_CALL_LIMITS.max_arg_len + 1);
	let err = program.call_export(export, &[big_arg], &[], None).expect_err("oversize arg should be rejected");
	assert!(matches!(err, ExecError::CallValidation(CallValidationError::ArgTooLong { .. })), "got: {err}");
}

#[test]
fn call_oversize_env_key_rejected() {
	use xeno_invocation::nu::DEFAULT_CALL_LIMITS;
	let (program, export) = varargs_program();
	let big_key = "K".repeat(DEFAULT_CALL_LIMITS.max_env_string_len + 1);
	let nothing = xeno_nu_data::Value::Nothing {
		internal_span: xeno_nu_data::Span::unknown(),
	};
	let env = [(big_key.as_str(), nothing)];
	let err = program.call_export(export, &[], &env, None).expect_err("oversize env key should be rejected");
	assert!(
		matches!(err, ExecError::CallValidation(CallValidationError::EnvKeyTooLong { .. })),
		"got: {err}"
	);
}

#[test]
fn call_over_max_env_vars_rejected() {
	use xeno_invocation::nu::DEFAULT_CALL_LIMITS;
	let (program, export) = varargs_program();
	let nothing = xeno_nu_data::Value::Nothing {
		internal_span: xeno_nu_data::Span::unknown(),
	};
	let env: Vec<(&str, xeno_nu_data::Value)> = (0..DEFAULT_CALL_LIMITS.max_env_vars + 1).map(|_| ("k", nothing.clone())).collect();
	let err = program.call_export(export, &[], &env, None).expect_err("over max_env_vars should be rejected");
	assert!(matches!(err, ExecError::CallValidation(CallValidationError::EnvTooMany { .. })), "got: {err}");
}

#[test]
fn call_oversize_env_nodes_rejected() {
	use xeno_invocation::nu::DEFAULT_CALL_LIMITS;
	let (program, export) = varargs_program();
	let nothing = xeno_nu_data::Value::Nothing {
		internal_span: xeno_nu_data::Span::unknown(),
	};
	let items: Vec<xeno_nu_data::Value> = (0..DEFAULT_CALL_LIMITS.max_env_nodes + 1).map(|_| nothing.clone()).collect();
	let big_val = xeno_nu_data::Value::List {
		vals: items,
		internal_span: xeno_nu_data::Span::unknown(),
	};
	let env = [("data", big_val)];
	let err = program.call_export(export, &[], &env, None).expect_err("oversize env nodes should be rejected");
	assert!(
		matches!(err, ExecError::CallValidation(CallValidationError::EnvValueTooComplex { .. })),
		"got: {err}"
	);
}

// --- Step 8.2: Host access tests ---

use crate::host::{BufferMeta, HostError, LineColRange, TextChunk, XenoNuHost};

struct MockHost;

impl XenoNuHost for MockHost {
	fn buffer_get(&self, _id: Option<i64>) -> Result<BufferMeta, HostError> {
		Ok(BufferMeta {
			path: Some("/tmp/test.rs".into()),
			file_type: Some("rust".into()),
			readonly: false,
			modified: true,
			line_count: 42,
		})
	}

	fn buffer_text(&self, _id: Option<i64>, range: Option<LineColRange>, max_bytes: usize) -> Result<TextChunk, HostError> {
		let full = "hello world\nsecond line\nthird line";
		let text = if let Some(r) = range {
			let lines: Vec<&str> = full.lines().collect();
			lines
				.get(r.start_line..=r.end_line.min(lines.len().saturating_sub(1)))
				.map(|s| s.join("\n"))
				.unwrap_or_default()
		} else {
			full.to_string()
		};
		let truncated = text.len() > max_bytes;
		let text = if truncated {
			// UTF-8 safe truncation: find the last valid char boundary at or before max_bytes
			let mut end = max_bytes;
			while end > 0 && !text.is_char_boundary(end) {
				end -= 1;
			}
			text[..end].to_string()
		} else {
			text
		};
		Ok(TextChunk { text, truncated })
	}
}

#[test]
fn host_buffer_get_returns_meta() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "export def test_meta [] { xeno buffer get }");
	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let export = program.resolve_export("test_meta").expect("should resolve");
	let host = MockHost;
	let value = program.call_export(export, &[], &[], Some(&host)).expect("call should succeed");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("path").unwrap().as_str().unwrap(), "/tmp/test.rs");
	assert_eq!(record.get("file_type").unwrap().as_str().unwrap(), "rust");
	assert_eq!(record.get("line_count").unwrap().as_int().unwrap(), 42);
	assert!(record.get("modified").unwrap().as_bool().unwrap());
}

#[test]
fn host_buffer_text_full() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "export def test_text [] { xeno buffer text }");
	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let export = program.resolve_export("test_text").expect("should resolve");
	let host = MockHost;
	let value = program.call_export(export, &[], &[], Some(&host)).expect("call should succeed");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("text").unwrap().as_str().unwrap(), "hello world\nsecond line\nthird line");
	assert!(!record.get("truncated").unwrap().as_bool().unwrap());
}

#[test]
fn host_buffer_text_ranged() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "export def test_text_range [] { xeno buffer text --start-line 1 --end-line 1 }");
	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let export = program.resolve_export("test_text_range").expect("should resolve");
	let host = MockHost;
	let value = program.call_export(export, &[], &[], Some(&host)).expect("call should succeed");
	let record = value.as_record().expect("should be record");
	assert_eq!(record.get("text").unwrap().as_str().unwrap(), "second line");
}

#[test]
fn host_buffer_get_without_host_errors() {
	let temp = tempfile::tempdir().expect("temp dir");
	write_script(temp.path(), "export def test_no_host [] { xeno buffer get }");
	let program = NuProgram::compile_macro_from_dir(temp.path()).expect("should compile");
	let export = program.resolve_export("test_no_host").expect("should resolve");
	let err = program.call_export(export, &[], &[], None).expect_err("should fail without host");
	assert!(matches!(err, ExecError::Runtime(_)));
}
