use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `length` implementation (no SQLite support).
#[derive(Clone)]
pub struct Length;

impl Command for Length {
	fn name(&self) -> &str {
		"length"
	}

	fn signature(&self) -> Signature {
		Signature::build("length")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::Int),
				(Type::table(), Type::Int),
				(Type::Nothing, Type::Int),
			])
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Count the number of items in an input list or rows in a table."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["count", "size", "wc"]
	}

	fn run(&self, engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let count = match input {
			PipelineData::Value(Value::List { vals, .. }, ..) => vals.len() as i64,
			PipelineData::Value(Value::Nothing { .. }, ..) | PipelineData::Empty => 0,
			PipelineData::Value(Value::Binary { val, .. }, ..) => val.len() as i64,
			PipelineData::Value(Value::String { val, .. }, ..) => val.len() as i64,
			PipelineData::ListStream(stream, ..) => {
				let mut count: i64 = 0;
				for _ in stream {
					engine_state.signals().check(&head)?;
					count += 1;
				}
				count
			}
			_ => {
				return Err(ShellError::OnlySupportsThisInputType {
					exp_input_type: "list, table, or nothing".into(),
					wrong_type: input.get_type().to_string(),
					dst_span: head,
					src_span: input.span().unwrap_or(head),
				});
			}
		};
		Ok(Value::int(count, head).into_pipeline_data())
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Count the number of items in a list.",
				example: "[1 2 3 4 5] | length",
				result: Some(Value::test_int(5)),
			},
			Example {
				description: "Count the number of rows in a table.",
				example: "[{a: 1} {a: 2}] | length",
				result: Some(Value::test_int(2)),
			},
		]
	}
}
