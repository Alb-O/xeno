use xeno_nu_engine::command_prelude::*;

use crate::limits::{MAX_ITEMS, collect_list_capped, err_limit};

#[derive(Clone)]
pub struct Prepend;

impl Command for Prepend {
	fn name(&self) -> &str {
		"prepend"
	}

	fn signature(&self) -> Signature {
		Signature::build("prepend")
			.input_output_types(vec![(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any)))])
			.required("value", SyntaxShape::Any, "The value to prepend.")
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Prepend a value to the beginning of a list."
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let value: Value = call.req(engine_state, stack, 0)?;

		let mut vals = collect_list_capped(input, head)?;

		if vals.len() >= MAX_ITEMS {
			return Err(err_limit(head, &format!("prepend would exceed {MAX_ITEMS} items")));
		}

		vals.insert(0, value);
		Ok(Value::list(vals, head).into_pipeline_data())
	}
}
