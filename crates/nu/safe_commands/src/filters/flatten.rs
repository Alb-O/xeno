use crate::limits::{MAX_ITEMS, collect_list_capped, err_limit};
use xeno_nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct Flatten;

impl Command for Flatten {
	fn name(&self) -> &str {
		"flatten"
	}

	fn signature(&self) -> Signature {
		Signature::build("flatten")
			.input_output_types(vec![(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any)))])
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Flatten a list of lists into a single list (one level)."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["concat", "join"]
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;

		let vals = collect_list_capped(input, head)?;

		let mut out = Vec::new();
		for item in vals {
			match item {
				Value::List { vals: inner, .. } => {
					for v in inner {
						out.push(v);
						if out.len() > MAX_ITEMS {
							return Err(err_limit(head, &format!("flatten output exceeds {MAX_ITEMS} items")));
						}
					}
				}
				Value::Error { .. } => out.push(item),
				other => {
					out.push(other);
					if out.len() > MAX_ITEMS {
						return Err(err_limit(head, &format!("flatten output exceeds {MAX_ITEMS} items")));
					}
				}
			}
		}

		Ok(Value::list(out, head).into_pipeline_data())
	}
}
