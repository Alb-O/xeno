use xeno_nu_engine::command_prelude::*;

use crate::limits::collect_list_capped;

#[derive(Clone)]
pub struct Compact;

impl Command for Compact {
	fn name(&self) -> &str {
		"compact"
	}

	fn signature(&self) -> Signature {
		Signature::build("compact")
			.input_output_types(vec![(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any)))])
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Remove null values from a list."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["filter", "null", "nothing"]
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;

		let vals = collect_list_capped(input, head)?;

		let out: Vec<Value> = vals.into_iter().filter(|v| !v.is_nothing()).collect();
		Ok(Value::list(out, head).into_pipeline_data())
	}
}
