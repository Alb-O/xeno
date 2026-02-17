use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `str starts-with` implementation.
#[derive(Clone)]
pub struct StrStartsWith;

impl Command for StrStartsWith {
	fn name(&self) -> &str {
		"str starts-with"
	}

	fn signature(&self) -> Signature {
		Signature::build("str starts-with")
			.input_output_types(vec![
				(Type::String, Type::Bool),
				(Type::List(Box::new(Type::String)), Type::List(Box::new(Type::Bool))),
			])
			.required("string", SyntaxShape::String, "The prefix to check.")
			.switch("ignore-case", "Comparison is case insensitive.", Some('i'))
			.category(Category::Strings)
	}

	fn description(&self) -> &str {
		"Check if an input starts with a string."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["prefix", "match", "find"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let prefix: String = call.req(engine_state, stack, 0)?;
		let case_insensitive = call.has_flag(engine_state, stack, "ignore-case")?;

		input.map(
			move |value| match &value {
				Value::String { val, .. } => {
					let found = if case_insensitive {
						val.to_lowercase().starts_with(&prefix.to_lowercase())
					} else {
						val.starts_with(prefix.as_str())
					};
					Value::bool(found, head)
				}
				Value::Error { .. } => value,
				_ => Value::error(
					ShellError::OnlySupportsThisInputType {
						exp_input_type: "string".into(),
						wrong_type: value.get_type().to_string(),
						dst_span: head,
						src_span: value.span(),
					},
					head,
				),
			},
			engine_state.signals(),
		)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![Example {
			description: "Check if input starts with prefix.",
			example: "'my_library.rb' | str starts-with 'my'",
			result: Some(Value::test_bool(true)),
		}]
	}
}
