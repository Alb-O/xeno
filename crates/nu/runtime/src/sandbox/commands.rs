/// Native Nu command declarations for invocation constructors and builtins.
///
/// Each invocation command returns a plain `Value::Record` with a `kind` field
/// identifying the invocation type. No custom values needed.
use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, Span, SyntaxShape, Type, Value};

// ---------------------------------------------------------------------------
// `action` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ActionCommand;

impl Command for ActionCommand {
	fn name(&self) -> &str {
		"action"
	}

	fn signature(&self) -> Signature {
		Signature::build("action")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Action name")
			.named("count", SyntaxShape::Int, "Repeat count", None)
			.switch("extend", "Extend selection", None)
			.named("register", SyntaxShape::String, "Register (single char)", None)
			.named("char", SyntaxShape::String, "Character argument (single char)", None)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create an action invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let count: Option<i64> = call.get_flag(engine_state, stack, "count")?;
		let extend = call.has_flag(engine_state, stack, "extend")?;
		let register: Option<String> = call.get_flag(engine_state, stack, "register")?;
		let char_arg: Option<String> = call.get_flag(engine_state, stack, "char")?;

		let count = count.map(|c| c.max(1)).unwrap_or(1);
		let register = parse_single_char(register, "register", span)?;
		let char_arg = parse_single_char(char_arg, "char", span)?;

		Ok(PipelineData::Value(schema::action_record(name, count, extend, register, char_arg, span), None))
	}
}

// ---------------------------------------------------------------------------
// `command` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CommandCommand;

impl Command for CommandCommand {
	fn name(&self) -> &str {
		"command"
	}

	fn signature(&self) -> Signature {
		Signature::build("command")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Command name")
			.rest("args", SyntaxShape::String, "Command arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create a command invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		Ok(PipelineData::Value(schema::command_record(name, args, span), None))
	}
}

// ---------------------------------------------------------------------------
// `editor` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EditorCommand;

impl Command for EditorCommand {
	fn name(&self) -> &str {
		"editor"
	}

	fn signature(&self) -> Signature {
		Signature::build("editor")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Editor command name")
			.rest("args", SyntaxShape::String, "Command arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create an editor command invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		Ok(PipelineData::Value(schema::editor_record(name, args, span), None))
	}
}

// ---------------------------------------------------------------------------
// `nu run` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct NuRunCommand;

impl Command for NuRunCommand {
	fn name(&self) -> &str {
		"nu run"
	}

	fn signature(&self) -> Signature {
		Signature::build("nu run")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Nu function name")
			.rest("args", SyntaxShape::String, "Function arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create a Nu macro invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		Ok(PipelineData::Value(schema::nu_record(name, args, span), None))
	}
}

// ---------------------------------------------------------------------------
// `xeno ctx` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct XenoCtxCommand;

impl Command for XenoCtxCommand {
	fn name(&self) -> &str {
		"xeno ctx"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno ctx")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Return the current Xeno invocation context (same as $env.XENO_CTX)"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = stack
			.get_env_var(engine_state, "XENO_CTX")
			.cloned()
			.unwrap_or_else(|| xeno_nu_protocol::Value::nothing(span));
		Ok(PipelineData::Value(value, None))
	}
}

// ---------------------------------------------------------------------------
// `xeno log` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct XenoLogCommand;

impl Command for XenoLogCommand {
	fn name(&self) -> &str {
		"xeno log"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno log")
			.input_output_types(vec![(Type::Any, Type::Any)])
			.required("label", SyntaxShape::String, "Log label for identifying the message")
			.named("level", SyntaxShape::String, "Log level: debug|info|warn|error (default: debug)", Some('l'))
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Log the pipeline value and pass it through unchanged"
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["debug", "print", "trace"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let label: String = call.req(engine_state, stack, 0)?;
		let level: Option<String> = call.get_flag(engine_state, stack, "level")?;
		let level_str = level.as_deref().unwrap_or("debug");

		match level_str {
			"debug" | "info" | "warn" | "error" => {}
			other => {
				return Err(ShellError::GenericError {
					error: format!("invalid log level: '{other}'"),
					msg: "expected one of: debug, info, warn, error".into(),
					span: Some(span),
					help: Some("valid levels: debug|info|warn|error".into()),
					inner: vec![],
				});
			}
		}

		let summary = match &input {
			PipelineData::Value(v, ..) => summarize_value(v),
			PipelineData::ListStream(..) => "<list stream>".to_string(),
			PipelineData::ByteStream(s, ..) => format!("<byte stream: {}>", s.type_().describe()),
			PipelineData::Empty => "<empty>".to_string(),
		};

		match level_str {
			"info" => tracing::info!(label = %label, "{summary}"),
			"warn" => tracing::warn!(label = %label, "{summary}"),
			"error" => tracing::error!(label = %label, "{summary}"),
			_ => tracing::debug!(label = %label, "{summary}"),
		}

		Ok(input)
	}
}

// ---------------------------------------------------------------------------
// `xeno assert` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct XenoAssertCommand;

impl Command for XenoAssertCommand {
	fn name(&self) -> &str {
		"xeno assert"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno assert")
			.input_output_types(vec![(Type::Any, Type::Any)])
			.required("predicate", SyntaxShape::Boolean, "Condition that must be true.")
			.optional(
				"message",
				SyntaxShape::String,
				"Error message if assertion fails (default: 'assertion failed').",
			)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Assert a condition; abort evaluation if false"
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["validate", "check", "guard"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let predicate: bool = call.req(engine_state, stack, 0)?;
		let message: Option<String> = call.opt(engine_state, stack, 1)?;
		let message = message.unwrap_or_else(|| "assertion failed".to_string());

		if predicate {
			Ok(input)
		} else {
			Err(ShellError::GenericError {
				error: "xeno assert failed".into(),
				msg: message,
				span: Some(span),
				help: Some("predicate evaluated to false".into()),
				inner: vec![],
			})
		}
	}
}

// ---------------------------------------------------------------------------
// `xeno emit` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct XenoEmitCommand;

impl Command for XenoEmitCommand {
	fn name(&self) -> &str {
		"xeno emit"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno emit")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("kind", SyntaxShape::String, "Invocation kind: action, command, editor, nu")
			.required("name", SyntaxShape::String, "Invocation name")
			.rest("args", SyntaxShape::String, "Arguments (for command/editor/nu kinds)")
			.named("count", SyntaxShape::Int, "Repeat count (action only)", None)
			.switch("extend", "Extend selection (action only)", None)
			.named("register", SyntaxShape::String, "Register, single char (action only)", None)
			.named("char", SyntaxShape::String, "Character argument, single char (action only)", None)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Emit a validated invocation record. Preferred over manual record construction."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["invoke", "dispatch", "record"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let kind: String = call.req(engine_state, stack, 0)?;
		let name: String = call.req(engine_state, stack, 1)?;

		if name.is_empty() {
			return Err(ShellError::GenericError {
				error: "xeno emit: name must not be empty".into(),
				msg: "empty name".into(),
				span: Some(span),
				help: None,
				inner: vec![],
			});
		}

		let record = match kind.as_str() {
			schema::KIND_ACTION => {
				let count: Option<i64> = call.get_flag(engine_state, stack, "count")?;
				let extend = call.has_flag(engine_state, stack, "extend")?;
				let register: Option<String> = call.get_flag(engine_state, stack, "register")?;
				let char_arg: Option<String> = call.get_flag(engine_state, stack, "char")?;
				let count = count.map(|c| c.max(1)).unwrap_or(1);
				let register = parse_single_char(register, "register", span)?;
				let char_arg = parse_single_char(char_arg, "char", span)?;
				schema::action_record(name, count, extend, register, char_arg, span)
			}
			schema::KIND_COMMAND | schema::KIND_EDITOR | schema::KIND_NU => {
				let limits = &schema::DEFAULT_LIMITS;
				let args: Vec<String> = call.rest(engine_state, stack, 2)?;
				if args.len() > limits.max_args {
					return Err(ShellError::GenericError {
						error: format!("xeno emit: too many args ({}, max {})", args.len(), limits.max_args),
						msg: "too many arguments".into(),
						span: Some(span),
						help: None,
						inner: vec![],
					});
				}
				for (i, arg) in args.iter().enumerate() {
					if arg.len() > limits.max_string_len {
						return Err(ShellError::GenericError {
							error: format!("xeno emit: arg[{i}] exceeds {} bytes", limits.max_string_len),
							msg: "argument too long".into(),
							span: Some(span),
							help: None,
							inner: vec![],
						});
					}
				}
				match kind.as_str() {
					schema::KIND_COMMAND => schema::command_record(name, args, span),
					schema::KIND_EDITOR => schema::editor_record(name, args, span),
					_ => schema::nu_record(name, args, span),
				}
			}
			other => {
				return Err(ShellError::GenericError {
					error: format!("xeno emit: unknown invocation kind '{other}'"),
					msg: "expected one of: action, command, editor, nu".into(),
					span: Some(span),
					help: Some("valid kinds: action, command, editor, nu".into()),
					inner: vec![],
				});
			}
		};

		Ok(PipelineData::Value(record, None))
	}
}

// ---------------------------------------------------------------------------
// `xeno emit-many` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct XenoEmitManyCommand;

impl Command for XenoEmitManyCommand {
	fn name(&self) -> &str {
		"xeno emit-many"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno emit-many")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
				(Type::Any, Type::List(Box::new(Type::Any))),
			])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Validate and normalize a list of invocation records. Accepts a single record or a list."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input.into_value(span).map_err(|e| ShellError::GenericError {
			error: format!("xeno emit-many: {e}"),
			msg: "failed to collect input".into(),
			span: Some(span),
			help: None,
			inner: vec![],
		})?;

		let items = match value {
			Value::Record { .. } => vec![value],
			Value::List { vals, .. } => vals,
			Value::Nothing { .. } => return Ok(PipelineData::Value(Value::list(vec![], span), None)),
			other => {
				return Err(ShellError::GenericError {
					error: "xeno emit-many: expected record or list of records".into(),
					msg: format!("got {}", other.get_type()),
					span: Some(span),
					help: None,
					inner: vec![],
				});
			}
		};

		let limits = &schema::DEFAULT_LIMITS;
		if items.len() > limits.max_invocations {
			return Err(ShellError::GenericError {
				error: format!("xeno emit-many: {} items exceeds limit of {}", items.len(), limits.max_invocations),
				msg: "too many invocations".into(),
				span: Some(span),
				help: None,
				inner: vec![],
			});
		}

		let mut out = Vec::with_capacity(items.len());
		for (idx, item) in items.into_iter().enumerate() {
			let rec = item.into_record().map_err(|_| ShellError::GenericError {
				error: format!("xeno emit-many: items[{idx}] must be a record"),
				msg: "expected record".into(),
				span: Some(span),
				help: None,
				inner: vec![],
			})?;
			let normalized = schema::validate_invocation_record(&rec, Some(idx), limits, span).map_err(|msg| ShellError::GenericError {
				error: format!("xeno emit-many: {msg}"),
				msg: msg.clone(),
				span: Some(span),
				help: None,
				inner: vec![],
			})?;
			out.push(normalized);
		}

		Ok(PipelineData::Value(Value::list(out, span), None))
	}
}

// ---------------------------------------------------------------------------
// `xeno is-invocation` command
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct XenoIsInvocationCommand;

impl Command for XenoIsInvocationCommand {
	fn name(&self) -> &str {
		"xeno is-invocation"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno is-invocation")
			.input_output_types(vec![(Type::Any, Type::Bool)])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Check if the pipeline value is a valid invocation record (has kind + name)."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input.into_value(span).map_err(|e| ShellError::GenericError {
			error: format!("xeno is-invocation: {e}"),
			msg: "failed to collect input".into(),
			span: Some(span),
			help: None,
			inner: vec![],
		})?;

		let is_invocation = match &value {
			Value::Record { val, .. } => {
				let kind_ok = val
					.get(schema::KIND)
					.and_then(|v| v.as_str().ok())
					.is_some_and(|k| matches!(k, "action" | "command" | "editor" | "nu"));
				let name_ok = val.get(schema::NAME).and_then(|v| v.as_str().ok()).is_some();
				kind_ok && name_ok
			}
			_ => false,
		};

		Ok(PipelineData::Value(Value::bool(is_invocation, span), None))
	}
}

const MAX_LOG_STRING: usize = 200;
const MAX_LOG_LIST: usize = 50;
const MAX_LOG_RECORD: usize = 50;
const MAX_LOG_NODES: usize = 200;
const MAX_LOG_OUT_BYTES: usize = 4096;

/// Truncate a string to at most `max_bytes` bytes at a valid UTF-8 char boundary.
/// Returns the truncated slice and whether truncation occurred.
fn trunc_utf8(s: &str, max_bytes: usize) -> (&str, bool) {
	if s.len() <= max_bytes {
		return (s, false);
	}
	// Find the last char boundary at or before max_bytes.
	let end = (0..=max_bytes).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
	(&s[..end], true)
}

fn summarize_value(v: &Value) -> String {
	let mut buf = String::new();
	let mut nodes = 0usize;
	summarize_inner(v, &mut buf, &mut nodes);
	if buf.len() > MAX_LOG_OUT_BYTES {
		let (trunc, _) = trunc_utf8(&buf, MAX_LOG_OUT_BYTES);
		let mut out = trunc.to_string();
		out.push_str("...");
		return out;
	}
	buf
}

fn summarize_inner(v: &Value, buf: &mut String, nodes: &mut usize) {
	use std::fmt::Write;
	*nodes += 1;
	if *nodes > MAX_LOG_NODES || buf.len() > MAX_LOG_OUT_BYTES {
		buf.push_str("...");
		return;
	}
	match v {
		Value::Nothing { .. } => buf.push_str("null"),
		Value::Bool { val, .. } => {
			let _ = write!(buf, "{val}");
		}
		Value::Int { val, .. } => {
			let _ = write!(buf, "{val}");
		}
		Value::Float { val, .. } => {
			let _ = write!(buf, "{val}");
		}
		Value::String { val, .. } => {
			buf.push('"');
			let (s, truncated) = trunc_utf8(val, MAX_LOG_STRING);
			buf.push_str(s);
			if truncated {
				buf.push_str("...");
			}
			buf.push('"');
		}
		Value::List { vals, .. } => {
			buf.push('[');
			let limit = vals.len().min(MAX_LOG_LIST);
			for (i, item) in vals.iter().take(limit).enumerate() {
				if i > 0 {
					buf.push_str(", ");
				}
				summarize_inner(item, buf, nodes);
				if *nodes > MAX_LOG_NODES || buf.len() > MAX_LOG_OUT_BYTES {
					break;
				}
			}
			if vals.len() > limit {
				let _ = write!(buf, ", ...+{}", vals.len() - limit);
			}
			buf.push(']');
		}
		Value::Record { val, .. } => {
			buf.push('{');
			let limit = val.len().min(MAX_LOG_RECORD);
			for (i, (k, v)) in val.iter().take(limit).enumerate() {
				if i > 0 {
					buf.push_str(", ");
				}
				let (key, key_trunc) = trunc_utf8(k, MAX_LOG_STRING);
				buf.push_str(key);
				if key_trunc {
					buf.push_str("...");
				}
				buf.push_str(": ");
				summarize_inner(v, buf, nodes);
				if *nodes > MAX_LOG_NODES || buf.len() > MAX_LOG_OUT_BYTES {
					break;
				}
			}
			if val.len() > limit {
				let _ = write!(buf, ", ...+{}", val.len() - limit);
			}
			buf.push('}');
		}
		Value::Error { error, .. } => {
			let _ = write!(buf, "<error: {error}>");
		}
		other => {
			let _ = write!(buf, "<{}>", other.get_type());
		}
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_single_char(value: Option<String>, flag: &str, span: Span) -> Result<Option<char>, ShellError> {
	let Some(s) = value else { return Ok(None) };
	let mut chars = s.chars();
	let Some(ch) = chars.next() else {
		return Err(ShellError::GenericError {
			error: format!("--{flag} must be exactly one character"),
			msg: "empty string".into(),
			span: Some(span),
			help: None,
			inner: vec![],
		});
	};
	if chars.next().is_some() {
		return Err(ShellError::GenericError {
			error: format!("--{flag} must be exactly one character"),
			msg: format!("got '{}' ({} chars)", s, s.chars().count()),
			span: Some(span),
			help: None,
			inner: vec![],
		});
	}
	Ok(Some(ch))
}

/// Register all xeno invocation commands into a working set.
pub fn register_all(working_set: &mut xeno_nu_protocol::engine::StateWorkingSet<'_>) {
	working_set.add_decl(Box::new(ActionCommand));
	working_set.add_decl(Box::new(CommandCommand));
	working_set.add_decl(Box::new(EditorCommand));
	working_set.add_decl(Box::new(NuRunCommand));
	working_set.add_decl(Box::new(XenoCtxCommand));
	working_set.add_decl(Box::new(XenoLogCommand));
	working_set.add_decl(Box::new(XenoAssertCommand));
	working_set.add_decl(Box::new(XenoEmitCommand));
	working_set.add_decl(Box::new(XenoEmitManyCommand));
	working_set.add_decl(Box::new(XenoIsInvocationCommand));
}
