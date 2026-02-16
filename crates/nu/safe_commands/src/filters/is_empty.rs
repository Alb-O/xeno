use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `is-empty` implementation.
#[derive(Clone)]
pub struct IsEmpty;

impl Command for IsEmpty {
	fn name(&self) -> &str {
		"is-empty"
	}

	fn signature(&self) -> Signature {
		Signature::build("is-empty")
			.input_output_types(vec![(Type::Any, Type::Bool)])
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Check for empty values."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["empty", "blank", "null"]
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let is_empty = match input {
			PipelineData::Empty | PipelineData::Value(Value::Nothing { .. }, ..) => true,
			PipelineData::Value(Value::String { val, .. }, ..) => val.is_empty(),
			PipelineData::Value(Value::List { vals, .. }, ..) => vals.is_empty(),
			PipelineData::Value(Value::Record { val, .. }, ..) => val.is_empty(),
			PipelineData::Value(Value::Binary { val, .. }, ..) => val.is_empty(),
			_ => false,
		};
		Ok(Value::bool(is_empty, head).into_pipeline_data())
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Check if a string is empty.",
				example: "'' | is-empty",
				result: Some(Value::test_bool(true)),
			},
			Example {
				description: "Check if a list is empty.",
				example: "[] | is-empty",
				result: Some(Value::test_bool(true)),
			},
		]
	}
}
