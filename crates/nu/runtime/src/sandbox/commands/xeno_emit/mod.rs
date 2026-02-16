use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type};

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
				let register = super::parse_single_char(register, "register", span)?;
				let char_arg = super::parse_single_char(char_arg, "char", span)?;
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

#[cfg(test)]
mod tests;
