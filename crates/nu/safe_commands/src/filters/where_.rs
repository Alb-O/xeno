use xeno_nu_engine::ClosureEval;
use xeno_nu_engine::command_prelude::*;
use xeno_nu_protocol::engine::{Closure, CommandType};

use crate::limits::{MAX_ITEMS, err_limit};

#[derive(Clone)]
pub struct Where;

impl Command for Where {
	fn name(&self) -> &str {
		"where"
	}

	fn description(&self) -> &str {
		"Filter values of an input list based on a condition."
	}

	fn command_type(&self) -> CommandType {
		CommandType::Keyword
	}

	fn signature(&self) -> Signature {
		Signature::build("where")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
				(Type::table(), Type::table()),
				(Type::Range, Type::Any),
			])
			.required("condition", SyntaxShape::RowCondition, "Filter row condition or closure.")
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["filter", "find", "search", "condition"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let closure: Closure = call.req(engine_state, stack, 0)?;
		let mut closure = ClosureEval::new(engine_state, stack, closure);
		let metadata = input.metadata();
		let mut count = 0usize;
		Ok(input
			.into_iter_strict(head)?
			.filter_map(move |value| {
				count += 1;
				if count > MAX_ITEMS {
					return Some(Value::error(err_limit(head, &format!("where iteration exceeds {MAX_ITEMS} items")), head));
				}
				match closure.run_with_value(value.clone()).and_then(|data| data.into_value(head)) {
					Ok(cond) => cond.is_true().then_some(value),
					Err(err) => Some(Value::error(err, head)),
				}
			})
			.into_pipeline_data_with_metadata(head, engine_state.signals().clone(), metadata))
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Filter rows of a table according to a condition.",
				example: "[{a: 1} {a: 2}] | where a > 1",
				result: Some(Value::test_list(vec![Value::test_record(record! {
					"a" => Value::test_int(2),
				})])),
			},
			Example {
				description: "Filter items of a list with a row condition.",
				example: "[1 2 3 4 5] | where $it > 2",
				result: Some(Value::test_list(vec![Value::test_int(3), Value::test_int(4), Value::test_int(5)])),
			},
		]
	}
}
