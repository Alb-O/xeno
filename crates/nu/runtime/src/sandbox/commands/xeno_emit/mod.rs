use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, SyntaxShape, Type, Value};

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

		let mut rec = Record::new();
		rec.push(schema::KIND, Value::string(kind.clone(), span));
		rec.push(schema::NAME, Value::string(name, span));

		match kind.as_str() {
			schema::KIND_ACTION => {
				let count: Option<i64> = call.get_flag(engine_state, stack, "count")?;
				let extend = call.has_flag(engine_state, stack, "extend")?;
				let register: Option<String> = call.get_flag(engine_state, stack, "register")?;
				let char_arg: Option<String> = call.get_flag(engine_state, stack, "char")?;
				if let Some(count) = count {
					rec.push(schema::COUNT, Value::int(count, span));
				}
				if extend {
					rec.push(schema::EXTEND, Value::bool(true, span));
				}
				if let Some(register) = register {
					rec.push(schema::REGISTER, Value::string(register, span));
				}
				if let Some(char_arg) = char_arg {
					rec.push(schema::CHAR, Value::string(char_arg, span));
				}
			}
			schema::KIND_COMMAND | schema::KIND_EDITOR | schema::KIND_NU => {
				let args: Vec<String> = call.rest(engine_state, stack, 2)?;
				rec.push(schema::ARGS, Value::list(args.into_iter().map(|arg| Value::string(arg, span)).collect(), span));
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
		}

		let normalized = schema::validate_invocation_record(&rec, None, &schema::DEFAULT_LIMITS, span).map_err(|msg| ShellError::GenericError {
			error: format!("xeno emit: {msg}"),
			msg: msg.clone(),
			span: Some(span),
			help: None,
			inner: vec![],
		})?;

		Ok(PipelineData::Value(normalized, None))
	}
}

#[cfg(test)]
mod tests;
