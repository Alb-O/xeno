/// Native Nu command declarations for invocation constructors.
///
/// Native [`Command`] implementations
/// registered into the engine state. Each command returns a
/// [`Value::Custom(InvocationValue)`] so decode is a trivial downcast.
use nu_engine::CallExt;
use nu_protocol::engine::{Call, Command, EngineState, Stack};
use nu_protocol::{Category, PipelineData, ShellError, Signature, Span, SyntaxShape, Type};

use xeno_invocation::Invocation;
use xeno_invocation::nu::InvocationValue;

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
			.input_output_types(vec![(Type::Nothing, Type::Custom("invocation".into()))])
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

		let count = count.map(|c| c.max(1) as usize).unwrap_or(1);
		let register = parse_single_char(register, "register", span)?;
		let char_arg = parse_single_char(char_arg, "char", span)?;

		let inv = if let Some(char_arg) = char_arg {
			Invocation::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			}
		} else {
			Invocation::Action { name, count, extend, register }
		};

		Ok(PipelineData::Value(InvocationValue(inv).into_value(span), None))
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
			.input_output_types(vec![(Type::Nothing, Type::Custom("invocation".into()))])
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
		let inv = Invocation::Command { name, args };
		Ok(PipelineData::Value(InvocationValue(inv).into_value(span), None))
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
			.input_output_types(vec![(Type::Nothing, Type::Custom("invocation".into()))])
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
		let inv = Invocation::EditorCommand { name, args };
		Ok(PipelineData::Value(InvocationValue(inv).into_value(span), None))
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
			.input_output_types(vec![(Type::Nothing, Type::Custom("invocation".into()))])
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
		let inv = Invocation::Nu { name, args };
		Ok(PipelineData::Value(InvocationValue(inv).into_value(span), None))
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
			.unwrap_or_else(|| nu_protocol::Value::nothing(span));
		Ok(PipelineData::Value(value, None))
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
pub fn register_all(working_set: &mut nu_protocol::engine::StateWorkingSet<'_>) {
	working_set.add_decl(Box::new(ActionCommand));
	working_set.add_decl(Box::new(CommandCommand));
	working_set.add_decl(Box::new(EditorCommand));
	working_set.add_decl(Box::new(NuRunCommand));
	working_set.add_decl(Box::new(XenoCtxCommand));
}
