use xeno_nu_engine::command_prelude::*;

use crate::strings::column_apply::{extract_column_names, map_columns_values};

#[derive(Clone)]
pub struct IntoBool;

impl Command for IntoBool {
	fn name(&self) -> &str {
		"into bool"
	}

	fn signature(&self) -> Signature {
		Signature::build("into bool")
			.input_output_types(vec![
				(Type::String, Type::Bool),
				(Type::Bool, Type::Bool),
				(Type::Int, Type::Bool),
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Bool))),
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
			])
			.rest("rest", SyntaxShape::CellPath, "For a record or table, columns to convert.")
			.allow_variants_without_examples(true)
			.category(Category::Conversions)
	}

	fn description(&self) -> &str {
		"Convert value to boolean."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["parse", "convert", "true", "false"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let columns: Vec<CellPath> = call.rest(engine_state, stack, 0)?;

		if !columns.is_empty() {
			let col_names = extract_column_names(&columns, head)?;
			return map_columns_values(input, &col_names, head, engine_state, move |v| convert_to_bool(v, head));
		}

		input.map(
			move |value| match convert_to_bool(&value, head) {
				Ok(v) => v,
				Err(e) => Value::error(e, head),
			},
			engine_state.signals(),
		)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Convert string to boolean.",
				example: "'true' | into bool",
				result: Some(Value::test_bool(true)),
			},
			Example {
				description: "Convert int to boolean.",
				example: "1 | into bool",
				result: Some(Value::test_bool(true)),
			},
		]
	}
}

fn convert_to_bool(value: &Value, head: Span) -> Result<Value, ShellError> {
	match value {
		Value::Bool { .. } => Ok(value.clone()),
		Value::Int { val, .. } => Ok(Value::bool(*val != 0, head)),
		Value::String { val, .. } => match val.trim().to_lowercase().as_str() {
			"true" | "yes" | "1" => Ok(Value::bool(true, head)),
			"false" | "no" | "0" => Ok(Value::bool(false, head)),
			other => Err(ShellError::CantConvert {
				to_type: "bool".into(),
				from_type: "string".into(),
				span: value.span(),
				help: Some(format!("cannot parse '{other}' as boolean (expected true/false/yes/no/1/0)")),
			}),
		},
		_ => Err(ShellError::CantConvert {
			to_type: "bool".into(),
			from_type: value.get_type().to_string(),
			span: value.span(),
			help: None,
		}),
	}
}
