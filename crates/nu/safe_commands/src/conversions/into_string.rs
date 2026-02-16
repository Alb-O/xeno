use crate::strings::column_apply::{extract_column_names, map_columns_values};
use xeno_nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct IntoString;

impl Command for IntoString {
	fn name(&self) -> &str {
		"into string"
	}

	fn signature(&self) -> Signature {
		Signature::build("into string")
			.input_output_types(vec![
				(Type::String, Type::String),
				(Type::Int, Type::String),
				(Type::Bool, Type::String),
				(Type::Float, Type::String),
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::String))),
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
			])
			.rest("rest", SyntaxShape::CellPath, "For a record or table, columns to convert.")
			.allow_variants_without_examples(true)
			.category(Category::Conversions)
	}

	fn description(&self) -> &str {
		"Convert value to string."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["parse", "convert", "text"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let columns: Vec<CellPath> = call.rest(engine_state, stack, 0)?;

		if !columns.is_empty() {
			let col_names = extract_column_names(&columns, head)?;
			return map_columns_values(input, &col_names, head, engine_state, move |v| convert_to_string(v, head));
		}

		input.map(
			move |value| match convert_to_string(&value, head) {
				Ok(v) => v,
				Err(e) => Value::error(e, head),
			},
			engine_state.signals(),
		)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Convert integer to string.",
				example: "42 | into string",
				result: Some(Value::test_string("42")),
			},
			Example {
				description: "Convert bool to string.",
				example: "true | into string",
				result: Some(Value::test_string("true")),
			},
		]
	}
}

fn convert_to_string(value: &Value, head: Span) -> Result<Value, ShellError> {
	match value {
		Value::String { .. } => Ok(value.clone()),
		Value::Int { val, .. } => Ok(Value::string(val.to_string(), head)),
		Value::Bool { val, .. } => Ok(Value::string(val.to_string(), head)),
		Value::Float { val, .. } => Ok(Value::string(val.to_string(), head)),
		_ => Err(ShellError::CantConvert {
			to_type: "string".into(),
			from_type: value.get_type().to_string(),
			span: value.span(),
			help: None,
		}),
	}
}
