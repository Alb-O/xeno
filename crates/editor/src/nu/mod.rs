//! Nu runtime for editor macro scripts.

use std::path::{Path, PathBuf};

use nu_protocol::Value;

use crate::types::Invocation;

const SCRIPT_FILE_NAME: &str = "xeno.nu";

/// Loaded Nu macro script runtime state.
#[derive(Debug, Clone)]
pub struct NuRuntime {
	config_dir: PathBuf,
	script_path: PathBuf,
	script_src: String,
}

impl NuRuntime {
	/// Load and validate the `xeno.nu` script from the given config directory.
	pub fn load(config_dir: &Path) -> Result<Self, String> {
		let script_path = config_dir.join(SCRIPT_FILE_NAME);
		let script_src = std::fs::read_to_string(&script_path).map_err(|error| format!("failed to read {}: {error}", script_path.display()))?;

		evaluate_script_only(config_dir, &script_path, &script_src)?;

		Ok(Self {
			config_dir: config_dir.to_path_buf(),
			script_path,
			script_src,
		})
	}

	/// Returns the loaded script path.
	pub fn script_path(&self) -> &Path {
		&self.script_path
	}

	/// Run a function in `xeno.nu` and return its raw Nu value.
	pub fn run(&self, fn_name: &str, args: &[String]) -> Result<Value, String> {
		self.run_internal(fn_name, args).map_err(map_run_error)
	}

	/// Run a function and decode its return value into invocation specs.
	pub fn run_invocation_specs(&self, fn_name: &str, args: &[String]) -> Result<Vec<String>, String> {
		let value = self.run_internal(fn_name, args).map_err(map_run_error)?;
		decode_invocation_specs(value)
	}

	/// Run a function and decode its return value into structured invocations.
	pub fn run_invocations(&self, fn_name: &str, args: &[String]) -> Result<Vec<Invocation>, String> {
		let value = self.run_internal(fn_name, args).map_err(map_run_error)?;
		decode_invocations(value)
	}

	/// Run a function and decode invocation specs, returning `None` when the function is absent.
	pub fn try_run_invocation_specs(&self, fn_name: &str, args: &[String]) -> Result<Option<Vec<String>>, String> {
		match self.run_internal(fn_name, args) {
			Ok(value) => decode_invocation_specs(value).map(Some),
			Err(NuRunError::MissingFunction(_)) => Ok(None),
			Err(NuRunError::Other(error)) => Err(error),
		}
	}

	/// Run a function and decode structured invocations, returning `None` when the function is absent.
	pub fn try_run_invocations(&self, fn_name: &str, args: &[String]) -> Result<Option<Vec<Invocation>>, String> {
		match self.run_internal(fn_name, args) {
			Ok(value) => decode_invocations(value).map(Some),
			Err(NuRunError::MissingFunction(_)) => Ok(None),
			Err(NuRunError::Other(error)) => Err(error),
		}
	}

	fn run_internal(&self, fn_name: &str, args: &[String]) -> Result<Value, NuRunError> {
		let mut engine_state = xeno_nu::create_engine_state(Some(&self.config_dir));
		let fname = self.script_path.to_string_lossy().to_string();

		let script_block = xeno_nu::parse_and_validate(&mut engine_state, &fname, &self.script_src, Some(&self.config_dir)).map_err(NuRunError::Other)?;
		xeno_nu::evaluate_block(&engine_state, script_block.as_ref()).map_err(NuRunError::Other)?;

		if engine_state.find_decl(fn_name.as_bytes(), &[]).is_none() {
			return Err(NuRunError::MissingFunction(fn_name.to_string()));
		}

		let call_src = xeno_nu::build_call_source(fn_name, args).map_err(NuRunError::Other)?;
		let call_block = xeno_nu::parse_and_validate(&mut engine_state, "<xeno.nu-run>", &call_src, Some(&self.config_dir)).map_err(NuRunError::Other)?;
		xeno_nu::evaluate_block(&engine_state, call_block.as_ref()).map_err(NuRunError::Other)
	}
}

#[derive(Debug)]
enum NuRunError {
	MissingFunction(String),
	Other(String),
}

fn map_run_error(error: NuRunError) -> String {
	match error {
		NuRunError::MissingFunction(name) => {
			format!("Nu runtime error: function '{name}' is not defined in xeno.nu")
		}
		NuRunError::Other(msg) => msg,
	}
}

fn evaluate_script_only(config_dir: &Path, script_path: &Path, script_src: &str) -> Result<(), String> {
	let mut engine_state = xeno_nu::create_engine_state(Some(config_dir));
	let fname = script_path.to_string_lossy().to_string();
	let block = xeno_nu::parse_and_validate(&mut engine_state, &fname, script_src, Some(config_dir))?;
	let _ = xeno_nu::evaluate_block(&engine_state, block.as_ref())?;
	Ok(())
}

fn decode_invocation_specs(value: Value) -> Result<Vec<String>, String> {
	match value {
		Value::String { val, .. } => Ok(vec![val]),
		Value::List { vals, .. } => decode_invocation_spec_list(vals),
		Value::Record { val, .. } => {
			let invocations = val
				.get("invocations")
				.ok_or_else(|| "Nu runtime error: record return must include 'invocations'".to_string())?;
			let list = invocations
				.as_list()
				.map_err(|_| "Nu runtime error: 'invocations' must be list<string>".to_string())?;
			decode_invocation_spec_list(list.to_vec())
		}
		other => Err(format!(
			"Nu runtime error: expected string, list<string>, or {{ invocations: list<string> }}, got {}",
			other.get_type()
		)),
	}
}

fn decode_invocation_spec_list(values: Vec<Value>) -> Result<Vec<String>, String> {
	let mut out = Vec::with_capacity(values.len());
	for (idx, value) in values.into_iter().enumerate() {
		match value {
			Value::String { val, .. } => out.push(val),
			other => {
				return Err(format!("Nu runtime error: invocation list item {idx} must be string, got {}", other.get_type()));
			}
		}
	}
	Ok(out)
}

/// Decode invocation return values from Nu macros and hooks.
pub fn decode_invocations(value: Value) -> Result<Vec<Invocation>, String> {
	match value {
		Value::String { val, .. } => Ok(vec![parse_invocation_spec(&val)?]),
		Value::List { vals, .. } => decode_invocation_values(vals),
		Value::Record { val, .. } => decode_invocation_record_or_wrapper(&val),
		other => Err(format!("Nu runtime error: expected invocation string/record/list, got {}", other.get_type())),
	}
}

fn decode_invocation_values(values: Vec<Value>) -> Result<Vec<Invocation>, String> {
	let mut out = Vec::new();
	for value in values {
		out.extend(decode_invocations(value)?);
	}
	Ok(out)
}

fn decode_invocation_record_or_wrapper(record: &nu_protocol::Record) -> Result<Vec<Invocation>, String> {
	if record.contains("kind") {
		return Ok(vec![decode_structured_invocation(record)?]);
	}

	if let Some(invocations) = record.get("invocations") {
		return match invocations.clone() {
			Value::List { vals, .. } => decode_invocation_values(vals),
			other => Err(format!("Nu runtime error: 'invocations' must be a list, got {}", other.get_type())),
		};
	}

	Err("Nu runtime error: record return must include either 'kind' or 'invocations'".to_string())
}

fn decode_structured_invocation(record: &nu_protocol::Record) -> Result<Invocation, String> {
	let kind = required_string_field(record, "kind")?;
	let name = required_string_field(record, "name")?;

	match kind.as_str() {
		"action" => {
			let count = optional_int_field(record, "count")?.unwrap_or(1).max(1);
			let extend = optional_bool_field(record, "extend")?.unwrap_or(false);
			let register = optional_char_field(record, "register")?;
			let char_arg = optional_char_field(record, "char")?;

			if let Some(char_arg) = char_arg {
				Ok(Invocation::ActionWithChar {
					name,
					count,
					extend,
					register,
					char_arg,
				})
			} else {
				Ok(Invocation::Action { name, count, extend, register })
			}
		}
		"command" => Ok(Invocation::Command {
			name,
			args: optional_string_list_field(record, "args")?.unwrap_or_default(),
		}),
		"editor" => Ok(Invocation::EditorCommand {
			name,
			args: optional_string_list_field(record, "args")?.unwrap_or_default(),
		}),
		other => Err(format!("Nu runtime error: unknown invocation kind '{other}'")),
	}
}

fn required_string_field(record: &nu_protocol::Record, field: &str) -> Result<String, String> {
	let value = record.get(field).ok_or_else(|| format!("Nu runtime error: missing required field '{field}'"))?;
	match value {
		Value::String { val, .. } => Ok(val.clone()),
		other => Err(format!("Nu runtime error: field '{field}' must be string, got {}", other.get_type())),
	}
}

fn optional_bool_field(record: &nu_protocol::Record, field: &str) -> Result<Option<bool>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	match value {
		Value::Bool { val, .. } => Ok(Some(*val)),
		other => Err(format!("Nu runtime error: field '{field}' must be bool, got {}", other.get_type())),
	}
}

fn optional_int_field(record: &nu_protocol::Record, field: &str) -> Result<Option<usize>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	match value {
		Value::Int { val, .. } => {
			if *val <= 0 {
				Ok(Some(1))
			} else {
				let max = usize::MAX as i128;
				let clamped = (*val as i128).min(max) as usize;
				Ok(Some(clamped))
			}
		}
		other => Err(format!("Nu runtime error: field '{field}' must be int, got {}", other.get_type())),
	}
}

fn optional_char_field(record: &nu_protocol::Record, field: &str) -> Result<Option<char>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	let s = match value {
		Value::String { val, .. } => val,
		other => {
			return Err(format!(
				"Nu runtime error: field '{field}' must be single-character string, got {}",
				other.get_type()
			));
		}
	};
	let mut chars = s.chars();
	let Some(ch) = chars.next() else {
		return Err(format!("Nu runtime error: field '{field}' must be exactly one character"));
	};
	if chars.next().is_some() {
		return Err(format!("Nu runtime error: field '{field}' must be exactly one character"));
	}
	Ok(Some(ch))
}

fn optional_string_list_field(record: &nu_protocol::Record, field: &str) -> Result<Option<Vec<String>>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	let list = match value {
		Value::List { vals, .. } => vals,
		other => {
			return Err(format!("Nu runtime error: field '{field}' must be list<string>, got {}", other.get_type()));
		}
	};

	let mut out = Vec::with_capacity(list.len());
	for (idx, item) in list.iter().enumerate() {
		match item {
			Value::String { val, .. } => out.push(val.clone()),
			other => {
				return Err(format!("Nu runtime error: field '{field}' item {idx} must be string, got {}", other.get_type()));
			}
		}
	}

	Ok(Some(out))
}

/// Parse a macro invocation spec string into an [`Invocation`].
pub fn parse_invocation_spec(spec: &str) -> Result<Invocation, String> {
	Invocation::parse_spec(spec)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn write_script(dir: &Path, source: &str) {
		std::fs::write(dir.join("xeno.nu"), source).expect("xeno.nu should be writable");
	}

	#[test]
	fn load_rejects_external_calls() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "^echo hi");
		let err = NuRuntime::load(temp.path()).expect_err("external calls should be rejected");
		let err_lower = err.to_lowercase();
		assert!(err_lower.contains("external") || err_lower.contains("parse error"), "{err}");
	}

	#[test]
	fn run_invocation_specs_supports_string_list_and_record() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(
			temp.path(),
			"export def one [] { \"editor:stats\" }\nexport def many [] { [\"editor:stats\", \"command:help\"] }\nexport def rec [] { { invocations: [\"editor:stats\"] } }",
		);

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

		let one = runtime.run_invocation_specs("one", &[]).expect("string return should decode");
		assert_eq!(one, vec!["editor:stats".to_string()]);

		let many = runtime.run_invocation_specs("many", &[]).expect("list return should decode");
		assert_eq!(many, vec!["editor:stats".to_string(), "command:help".to_string()]);

		let rec = runtime.run_invocation_specs("rec", &[]).expect("record return should decode");
		assert_eq!(rec, vec!["editor:stats".to_string()]);
	}

	#[test]
	fn run_invocations_supports_structured_records() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(
			temp.path(),
			"export def action_rec [] { { kind: \"action\", name: \"move_right\", count: 2, extend: true, register: \"a\" } }\n\
export def action_char [] { { kind: \"action\", name: \"find_char\", char: \"x\" } }\n\
export def mixed [] { [ { kind: \"editor\", name: \"stats\" }, { kind: \"command\", name: \"help\", args: [\"themes\"] } ] }\n\
export def wrapped [] { { invocations: [ { kind: \"editor\", name: \"stats\" } ] } }",
		);

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");

		let action = runtime.run_invocations("action_rec", &[]).expect("structured action should decode");
		assert!(matches!(
			action.as_slice(),
			[Invocation::Action {
				name,
				count: 2,
				extend: true,
				register: Some('a')
			}] if name == "move_right"
		));

		let action_char = runtime.run_invocations("action_char", &[]).expect("structured action-with-char should decode");
		assert!(matches!(
			action_char.as_slice(),
			[Invocation::ActionWithChar {
				name,
				char_arg: 'x',
				..
			}] if name == "find_char"
		));

		let mixed = runtime.run_invocations("mixed", &[]).expect("structured list should decode");
		assert!(matches!(mixed.first(), Some(Invocation::EditorCommand { name, .. }) if name == "stats"));
		assert!(matches!(mixed.get(1), Some(Invocation::Command { name, .. }) if name == "help"));

		let wrapped = runtime.run_invocations("wrapped", &[]).expect("wrapped structured list should decode");
		assert!(matches!(wrapped.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
	}

	#[test]
	fn run_allows_use_within_config_root() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		std::fs::write(temp.path().join("mod.nu"), "export def mk [] { \"editor:stats\" }").expect("module should be writable");
		write_script(temp.path(), "use mod.nu *\nexport def go [] { mk }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let specs = runtime.run_invocation_specs("go", &[]).expect("run should succeed");
		assert_eq!(specs, vec!["editor:stats".to_string()]);
	}

	#[test]
	fn try_run_returns_none_for_missing_function() {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		write_script(temp.path(), "export def known [] { \"editor:stats\" }");

		let runtime = NuRuntime::load(temp.path()).expect("runtime should load");
		let missing = runtime.try_run_invocations("missing", &[]).expect("missing function should be non-fatal");
		assert!(missing.is_none());
	}
}
