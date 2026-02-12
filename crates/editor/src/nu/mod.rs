//! Nu runtime for editor macro scripts.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use nu_protocol::ast::{Argument, Block, Call, Expr, Expression, ListItem, RecordItem};
use nu_protocol::engine::{Stack, StateWorkingSet};
use nu_protocol::{BlockId, PipelineData, Span, Value};

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
		let mut engine_state = create_engine_state(&self.config_dir);

		let script_block = parse_and_merge_script(&mut engine_state, &self.script_path, &self.script_src, &self.config_dir).map_err(NuRunError::Other)?;
		evaluate_block(&engine_state, script_block.as_ref()).map_err(NuRunError::Other)?;

		if engine_state.find_decl(fn_name.as_bytes(), &[]).is_none() {
			return Err(NuRunError::MissingFunction(fn_name.to_string()));
		}

		let call_src = build_call_source(fn_name, args).map_err(NuRunError::Other)?;
		let call_block = parse_and_merge_call(&mut engine_state, &call_src, &self.config_dir).map_err(NuRunError::Other)?;
		evaluate_block(&engine_state, call_block.as_ref()).map_err(NuRunError::Other)
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
	let mut engine_state = create_engine_state(config_dir);
	let block = parse_and_merge_script(&mut engine_state, script_path, script_src, config_dir)?;
	let _ = evaluate_block(&engine_state, block.as_ref())?;
	Ok(())
}

fn create_engine_state(config_dir: &Path) -> nu_protocol::engine::EngineState {
	let mut engine_state = nu_cmd_lang::create_default_context();
	if let Ok(cwd) = std::fs::canonicalize(config_dir) {
		engine_state.add_env_var("PWD".to_string(), Value::string(cwd.to_string_lossy().to_string(), Span::unknown()));
	}
	engine_state
}

fn parse_and_merge_script(
	engine_state: &mut nu_protocol::engine::EngineState,
	script_path: &Path,
	script_src: &str,
	config_root: &Path,
) -> Result<std::sync::Arc<Block>, String> {
	let mut working_set = StateWorkingSet::new(engine_state);
	let fname = script_path.to_string_lossy().to_string();
	let block = nu_parser::parse(&mut working_set, Some(&fname), script_src.as_bytes(), false);
	validate_parse_state(&working_set)?;
	ensure_working_set_is_sandboxed(&working_set, block.as_ref(), Some(config_root))?;

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|error| format!("Nu parse error: {error}"))?;

	Ok(block)
}

fn parse_and_merge_call(engine_state: &mut nu_protocol::engine::EngineState, call_src: &str, config_root: &Path) -> Result<std::sync::Arc<Block>, String> {
	let mut working_set = StateWorkingSet::new(engine_state);
	let block = nu_parser::parse(&mut working_set, Some("<xeno.nu-run>"), call_src.as_bytes(), false);
	validate_parse_state(&working_set)?;
	ensure_working_set_is_sandboxed(&working_set, block.as_ref(), Some(config_root))?;

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|error| format!("Nu parse error: {error}"))?;

	Ok(block)
}

fn validate_parse_state(working_set: &StateWorkingSet<'_>) -> Result<(), String> {
	if let Some(error) = working_set.parse_errors.first() {
		return Err(format!("Nu parse error: {error}"));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(format!("Nu parse error: {error}"));
	}
	Ok(())
}

fn evaluate_block(engine_state: &nu_protocol::engine::EngineState, block: &Block) -> Result<Value, String> {
	let mut stack = Stack::new();
	let eval_block = nu_engine::get_eval_block(engine_state);
	let execution = eval_block(engine_state, &mut stack, block, PipelineData::empty()).map_err(|error| format!("Nu runtime error: {error}"))?;
	execution.body.into_value(Span::unknown()).map_err(|error| format!("Nu runtime error: {error}"))
}

fn build_call_source(fn_name: &str, args: &[String]) -> Result<String, String> {
	if fn_name.is_empty() {
		return Err("Nu runtime error: function name cannot be empty".to_string());
	}
	if !fn_name.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')) {
		return Err("Nu runtime error: function name contains unsupported characters".to_string());
	}

	let mut src = fn_name.to_string();
	for arg in args {
		src.push(' ');
		src.push_str(&quote_nu_string(arg));
	}
	Ok(src)
}

fn quote_nu_string(input: &str) -> String {
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
	let spec = spec.trim();
	if spec.is_empty() {
		return Err("empty invocation spec".to_string());
	}

	if let Some(action) = spec.strip_prefix("action:") {
		let action = action.trim();
		if action.is_empty() {
			return Err("action invocation missing target".to_string());
		}
		if action.contains(char::is_whitespace) {
			return Err(format!("action invocation must not include spaces: {spec}"));
		}
		return Ok(Invocation::action(action));
	}

	if let Some(command) = spec.strip_prefix("command:") {
		let mut parts = command.split_whitespace();
		let name = parts.next().ok_or_else(|| "command invocation missing command name".to_string())?;
		let args = parts.map(str::to_string).collect();
		return Ok(Invocation::command(name, args));
	}

	if let Some(command) = spec.strip_prefix("editor:") {
		let mut parts = command.split_whitespace();
		let name = parts.next().ok_or_else(|| "editor invocation missing command name".to_string())?;
		let args = parts.map(str::to_string).collect();
		return Ok(Invocation::editor_command(name, args));
	}

	Err(format!("unsupported invocation spec '{spec}', expected action:/command:/editor:"))
}

fn ensure_working_set_is_sandboxed(working_set: &StateWorkingSet<'_>, root: &Block, config_root: Option<&Path>) -> Result<(), String> {
	let mut visited_blocks = HashSet::new();
	check_block(working_set, root, &mut visited_blocks, config_root)?;

	let base = working_set.permanent_state.num_blocks();
	for idx in 0..working_set.delta.blocks.len() {
		let block_id = BlockId::new(base + idx);
		check_block_by_id(working_set, block_id, &mut visited_blocks, config_root)?;
	}

	Ok(())
}

fn check_block_by_id(
	working_set: &StateWorkingSet<'_>,
	block_id: BlockId,
	visited_blocks: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<(), String> {
	if !visited_blocks.insert(block_id) {
		return Ok(());
	}

	let block = working_set.get_block(block_id);
	check_block(working_set, block, visited_blocks, config_root)
}

fn check_block(working_set: &StateWorkingSet<'_>, block: &Block, visited_blocks: &mut HashSet<BlockId>, config_root: Option<&Path>) -> Result<(), String> {
	for pipeline in &block.pipelines {
		for element in &pipeline.elements {
			check_expression(working_set, &element.expr, visited_blocks, config_root)?;
			if element.redirection.is_some() {
				return Err("Nu sandbox error: pipeline redirection is disabled in xeno.nu".to_string());
			}
		}
	}

	Ok(())
}

fn check_expression(
	working_set: &StateWorkingSet<'_>,
	expression: &Expression,
	visited_blocks: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<(), String> {
	match &expression.expr {
		Expr::ExternalCall(_, _) => Err("Nu sandbox error: external commands are disabled in xeno.nu".to_string()),
		Expr::Call(call) => {
			let decl_name = working_set.get_decl(call.decl_id).name();
			if is_use_decl(decl_name) {
				return check_use_call(working_set, call, visited_blocks, config_root);
			}
			if let Some(reason) = blocked_decl_reason(decl_name) {
				return Err(format!("Nu sandbox error: '{decl_name}' is not allowed in xeno.nu ({reason})"));
			}

			for arg in &call.arguments {
				match arg {
					Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
						check_expression(working_set, expr, visited_blocks, config_root)?;
					}
					Argument::Named((_, _, maybe_expr)) => {
						if let Some(expr) = maybe_expr {
							check_expression(working_set, expr, visited_blocks, config_root)?;
						}
					}
				}
			}
			for expr in call.parser_info.values() {
				check_expression(working_set, expr, visited_blocks, config_root)?;
			}

			Ok(())
		}
		Expr::AttributeBlock(attribute_block) => {
			for attribute in &attribute_block.attributes {
				check_expression(working_set, &attribute.expr, visited_blocks, config_root)?;
			}
			check_expression(working_set, &attribute_block.item, visited_blocks, config_root)
		}
		Expr::UnaryNot(expr) => check_expression(working_set, expr, visited_blocks, config_root),
		Expr::BinaryOp(lhs, op, rhs) => {
			check_expression(working_set, lhs, visited_blocks, config_root)?;
			check_expression(working_set, op, visited_blocks, config_root)?;
			check_expression(working_set, rhs, visited_blocks, config_root)
		}
		Expr::Collect(_, expr) => check_expression(working_set, expr, visited_blocks, config_root),
		Expr::Subexpression(block_id) | Expr::Block(block_id) | Expr::Closure(block_id) | Expr::RowCondition(block_id) => {
			check_block_by_id(working_set, *block_id, visited_blocks, config_root)
		}
		Expr::MatchBlock(cases) => {
			for (_, expr) in cases {
				check_expression(working_set, expr, visited_blocks, config_root)?;
			}
			Ok(())
		}
		Expr::List(list) => {
			for item in list {
				match item {
					ListItem::Item(expr) | ListItem::Spread(_, expr) => {
						check_expression(working_set, expr, visited_blocks, config_root)?;
					}
				}
			}
			Ok(())
		}
		Expr::Record(items) => {
			for item in items {
				match item {
					RecordItem::Pair(key, value) => {
						check_expression(working_set, key, visited_blocks, config_root)?;
						check_expression(working_set, value, visited_blocks, config_root)?;
					}
					RecordItem::Spread(_, value) => {
						check_expression(working_set, value, visited_blocks, config_root)?;
					}
				}
			}
			Ok(())
		}
		Expr::Keyword(keyword) => check_expression(working_set, &keyword.expr, visited_blocks, config_root),
		Expr::ValueWithUnit(value_with_unit) => check_expression(working_set, &value_with_unit.expr, visited_blocks, config_root),
		Expr::FullCellPath(path) => check_expression(working_set, &path.head, visited_blocks, config_root),
		Expr::GlobPattern(_, _) | Expr::GlobInterpolation(_, _) => Err("Nu sandbox error: glob expansion is disabled in xeno.nu".to_string()),
		Expr::StringInterpolation(items) => {
			for item in items {
				check_expression(working_set, item, visited_blocks, config_root)?;
			}
			Ok(())
		}
		Expr::Range(range) => {
			if let Some(from) = &range.from {
				check_expression(working_set, from, visited_blocks, config_root)?;
			}
			if let Some(next) = &range.next {
				check_expression(working_set, next, visited_blocks, config_root)?;
			}
			if let Some(to) = &range.to {
				check_expression(working_set, to, visited_blocks, config_root)?;
			}
			Ok(())
		}
		Expr::Table(table) => {
			for column in table.columns.iter() {
				check_expression(working_set, column, visited_blocks, config_root)?;
			}
			for row in table.rows.iter() {
				for cell in row.iter() {
					check_expression(working_set, cell, visited_blocks, config_root)?;
				}
			}
			Ok(())
		}
		_ => Ok(()),
	}
}

fn is_use_decl(decl_name: &str) -> bool {
	matches!(decl_name, "use" | "export use")
}

fn check_use_call(working_set: &StateWorkingSet<'_>, call: &Call, visited_blocks: &mut HashSet<BlockId>, config_root: Option<&Path>) -> Result<(), String> {
	let Some((module_index, module_expr)) = call.arguments.iter().enumerate().find_map(|(idx, arg)| match arg {
		Argument::Positional(expr) | Argument::Unknown(expr) => Some((idx, expr)),
		Argument::Named(_) | Argument::Spread(_) => None,
	}) else {
		return Err("Nu sandbox error: use requires a static module path literal".to_string());
	};

	let raw_path = module_path_literal(module_expr).ok_or_else(|| "Nu sandbox error: use module path must be a static path literal".to_string())?;
	validate_module_path(config_root, raw_path)?;

	for (idx, arg) in call.arguments.iter().enumerate() {
		if idx == module_index {
			continue;
		}
		match arg {
			Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
				check_expression(working_set, expr, visited_blocks, config_root)?;
			}
			Argument::Named((_, _, maybe_expr)) => {
				if let Some(expr) = maybe_expr {
					check_expression(working_set, expr, visited_blocks, config_root)?;
				}
			}
		}
	}
	for expr in call.parser_info.values() {
		check_expression(working_set, expr, visited_blocks, config_root)?;
	}

	Ok(())
}

fn module_path_literal(expr: &Expression) -> Option<&str> {
	match &expr.expr {
		Expr::String(path) | Expr::Filepath(path, _) | Expr::GlobPattern(path, _) => Some(path),
		_ => None,
	}
}

fn validate_module_path(config_root: Option<&Path>, raw_path: &str) -> Result<(), String> {
	if raw_path.is_empty() {
		return Err("Nu sandbox error: use module path cannot be empty".to_string());
	}
	if raw_path.contains('\0') {
		return Err("Nu sandbox error: use module path contains NUL byte".to_string());
	}
	if raw_path.contains('~') || raw_path.contains('$') || raw_path.contains('`') {
		return Err("Nu sandbox error: use module path must not use shell expansion tokens".to_string());
	}
	if raw_path.chars().any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}')) {
		return Err("Nu sandbox error: use module path must not contain glob patterns".to_string());
	}

	let path = Path::new(raw_path);
	if path.is_absolute() {
		return Err("Nu sandbox error: absolute module paths are not allowed in xeno.nu".to_string());
	}
	for comp in path.components() {
		match comp {
			Component::ParentDir => {
				return Err("Nu sandbox error: module paths cannot traverse parent directories".to_string());
			}
			Component::Prefix(_) | Component::RootDir => {
				return Err("Nu sandbox error: module path has an unsupported root or prefix".to_string());
			}
			Component::CurDir | Component::Normal(_) => {}
		}
	}
	if path.extension().and_then(|ext| ext.to_str()) != Some("nu") {
		return Err("Nu sandbox error: module path must point to a .nu file".to_string());
	}

	let config_root = config_root.ok_or_else(|| "Nu sandbox error: use requires a real config directory path".to_string())?;
	let root_canon = std::fs::canonicalize(config_root).map_err(|e| format!("Nu sandbox error: failed to resolve config directory root: {e}"))?;
	let candidate_canon =
		std::fs::canonicalize(config_root.join(path)).map_err(|e| format!("Nu sandbox error: failed to resolve module path '{raw_path}': {e}"))?;
	if !candidate_canon.starts_with(&root_canon) {
		return Err("Nu sandbox error: module path resolves outside the config directory root".to_string());
	}
	let metadata = std::fs::metadata(&candidate_canon).map_err(|e| format!("Nu sandbox error: failed to stat module path '{raw_path}': {e}"))?;
	if !metadata.is_file() {
		return Err("Nu sandbox error: module path must resolve to a file".to_string());
	}

	Ok(())
}

fn blocked_decl_reason(decl_name: &str) -> Option<&'static str> {
	let name = decl_name.to_ascii_lowercase();
	match name.as_str() {
		"run-external" => Some("external execution is disabled"),
		"source" | "source-env" | "overlay use" | "overlay new" | "overlay hide" => Some("module and filesystem loading are disabled"),
		"for" | "while" | "loop" => Some("looping commands are disabled"),
		"exec" | "bash" | "sh" | "nu" | "cmd" | "powershell" | "pwsh" => Some("process execution commands are disabled"),
		"open" | "save" | "rm" | "mv" | "cp" | "mkdir" | "ls" | "cd" => Some("filesystem commands are disabled"),
		"http" | "curl" | "wget" => Some("network commands are disabled"),
		"plugin" | "register" | "plugin use" => Some("plugin commands are disabled"),
		_ => None,
	}
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
		assert!(err.contains("Nu sandbox error") || err.contains("Nu parse error"));
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
