use crate::limits::{MAX_ITEMS, collect_list_capped, err_limit};
use xeno_nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct Append;

impl Command for Append {
	fn name(&self) -> &str {
		"append"
	}

	fn signature(&self) -> Signature {
		Signature::build("append")
			.input_output_types(vec![(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any)))])
			.required("value", SyntaxShape::Any, "The value to append.")
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Append a value to the end of a list."
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let value: Value = call.req(engine_state, stack, 0)?;

		let mut vals = collect_list_capped(input, head)?;

		if vals.len() >= MAX_ITEMS {
			return Err(err_limit(head, &format!("append would exceed {MAX_ITEMS} items")));
		}

		vals.push(value);
		Ok(Value::list(vals, head).into_pipeline_data())
	}
}
