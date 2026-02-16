use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `str contains` implementation (no cell path traversal, no nu-cmd-base).
#[derive(Clone)]
pub struct StrContains;

impl Command for StrContains {
	fn name(&self) -> &str {
		"str contains"
	}

	fn signature(&self) -> Signature {
		Signature::build("str contains")
			.input_output_types(vec![
				(Type::String, Type::Bool),
				(Type::List(Box::new(Type::String)), Type::List(Box::new(Type::Bool))),
			])
			.required("string", SyntaxShape::String, "The substring to find.")
			.switch("ignore-case", "Search is case insensitive.", Some('i'))
			.category(Category::Strings)
	}

	fn description(&self) -> &str {
		"Checks if string input contains a substring."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["substring", "match", "find", "search"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let substring: String = call.req(engine_state, stack, 0)?;
		let case_insensitive = call.has_flag(engine_state, stack, "ignore-case")?;

		input.map(
			move |value| match &value {
				Value::String { val, .. } => {
					let found = if case_insensitive {
						val.to_lowercase().contains(&substring.to_lowercase())
					} else {
						val.contains(substring.as_str())
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
		vec![
			Example {
				description: "Check if input contains string.",
				example: "'my_library.rb' | str contains '.rb'",
				result: Some(Value::test_bool(true)),
			},
			Example {
				description: "Check if input contains string case insensitive.",
				example: "'my_library.rb' | str contains --ignore-case '.RB'",
				result: Some(Value::test_bool(true)),
			},
		]
	}
}
