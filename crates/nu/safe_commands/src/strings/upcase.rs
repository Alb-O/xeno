use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `str upcase` implementation.
#[derive(Clone)]
pub struct StrUpcase;

impl Command for StrUpcase {
	fn name(&self) -> &str {
		"str upcase"
	}

	fn signature(&self) -> Signature {
		Signature::build("str upcase")
			.input_output_types(vec![
				(Type::String, Type::String),
				(Type::List(Box::new(Type::String)), Type::List(Box::new(Type::String))),
			])
			.category(Category::Strings)
	}

	fn description(&self) -> &str {
		"Make text uppercase."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["uppercase", "upper"]
	}

	fn run(&self, engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;

		input.map(
			move |value| match &value {
				Value::String { val, .. } => Value::string(val.to_uppercase(), head),
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
			description: "Upcase contents.",
			example: "'hello' | str upcase",
			result: Some(Value::test_string("HELLO")),
		}]
	}
}
