use crate::strings::column_apply::{extract_column_names, map_columns_values};
use xeno_nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct IntoInt;

impl Command for IntoInt {
	fn name(&self) -> &str {
		"into int"
	}

	fn signature(&self) -> Signature {
		Signature::build("into int")
			.input_output_types(vec![
				(Type::String, Type::Int),
				(Type::Int, Type::Int),
				(Type::Bool, Type::Int),
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Int))),
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
			])
			.rest("rest", SyntaxShape::CellPath, "For a record or table, columns to convert.")
			.allow_variants_without_examples(true)
			.category(Category::Conversions)
	}

	fn description(&self) -> &str {
		"Convert value to integer."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["parse", "number", "convert"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let columns: Vec<CellPath> = call.rest(engine_state, stack, 0)?;

		if !columns.is_empty() {
			let col_names = extract_column_names(&columns, head)?;
			return map_columns_values(input, &col_names, head, engine_state, move |v| convert_to_int(v, head));
		}

		input.map(
			move |value| match convert_to_int(&value, head) {
				Ok(v) => v,
				Err(e) => Value::error(e, head),
			},
			engine_state.signals(),
		)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Convert string to integer.",
				example: "'42' | into int",
				result: Some(Value::test_int(42)),
			},
			Example {
				description: "Convert bool to integer.",
				example: "true | into int",
				result: Some(Value::test_int(1)),
			},
		]
	}
}

fn convert_to_int(value: &Value, head: Span) -> Result<Value, ShellError> {
	match value {
		Value::Int { .. } => Ok(value.clone()),
		Value::Bool { val, .. } => Ok(Value::int(if *val { 1 } else { 0 }, head)),
		Value::String { val, .. } => {
			let trimmed = val.trim();
			trimmed.parse::<i64>().map(|n| Value::int(n, head)).map_err(|_| ShellError::CantConvert {
				to_type: "int".into(),
				from_type: "string".into(),
				span: value.span(),
				help: Some(format!("cannot parse '{trimmed}' as integer")),
			})
		}
		_ => Err(ShellError::CantConvert {
			to_type: "int".into(),
			from_type: value.get_type().to_string(),
			span: value.span(),
			help: None,
		}),
	}
}
