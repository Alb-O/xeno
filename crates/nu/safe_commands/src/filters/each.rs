use crate::limits::{MAX_ITEMS, err_limit};
use xeno_nu_engine::{ClosureEval, ClosureEvalOnce, command_prelude::*};
use xeno_nu_protocol::engine::Closure;

#[derive(Clone)]
pub struct Each;

impl Command for Each {
	fn name(&self) -> &str {
		"each"
	}

	fn description(&self) -> &str {
		"Run a closure on each row of the input list, creating a new list with the results."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["for", "loop", "iterate", "map"]
	}

	fn signature(&self) -> Signature {
		Signature::build("each")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
				(Type::table(), Type::List(Box::new(Type::Any))),
				(Type::Any, Type::Any),
			])
			.required("closure", SyntaxShape::Closure(Some(vec![SyntaxShape::Any])), "The closure to run.")
			.switch("keep-empty", "Keep empty result cells.", Some('k'))
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				example: "[1 2 3] | each {|e| 2 * $e }",
				description: "Multiplies elements in the list.",
				result: Some(Value::test_list(vec![Value::test_int(2), Value::test_int(4), Value::test_int(6)])),
			},
			Example {
				example: r#"[1 2 3 2] | each {|e| if $e == 2 { "two" } }"#,
				description: "Null items are dropped from the result list (like filter_map).",
				result: Some(Value::test_list(vec![Value::test_string("two"), Value::test_string("two")])),
			},
		]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let closure: Closure = call.req(engine_state, stack, 0)?;
		let keep_empty = call.has_flag(engine_state, stack, "keep-empty")?;

		let metadata = input.metadata();
		let result = match input {
			empty @ (PipelineData::Empty | PipelineData::Value(Value::Nothing { .. }, ..)) => return Ok(empty),
			PipelineData::Value(Value::List { ref vals, .. }, ..) => {
				if vals.len() > MAX_ITEMS {
					return Err(err_limit(head, &format!("list exceeds {MAX_ITEMS} items")));
				}
				let mut closure = ClosureEval::new(engine_state, stack, closure);
				let mut count = 0usize;
				let out = input
					.into_iter()
					.map(move |value| {
						count += 1;
						if count > MAX_ITEMS {
							return Value::error(err_limit(head, &format!("iteration exceeds {MAX_ITEMS} items")), head);
						}
						each_map(value, &mut closure, head).unwrap_or_else(|error| Value::error(error, head))
					})
					.into_pipeline_data(head, engine_state.signals().clone());
				Ok(out)
			}
			PipelineData::ListStream(..) => {
				let mut closure = ClosureEval::new(engine_state, stack, closure);
				let mut count = 0usize;
				let out = input
					.into_iter()
					.map(move |value| {
						count += 1;
						if count > MAX_ITEMS {
							return Value::error(err_limit(head, &format!("iteration exceeds {MAX_ITEMS} items")), head);
						}
						each_map(value, &mut closure, head).unwrap_or_else(|error| Value::error(error, head))
					})
					.into_pipeline_data(head, engine_state.signals().clone());
				Ok(out)
			}
			PipelineData::ByteStream(stream, ..) => {
				let Some(chunks) = stream.chunks() else {
					return Ok(PipelineData::empty().set_metadata(metadata));
				};
				let mut closure = ClosureEval::new(engine_state, stack, closure);
				let mut count = 0usize;
				let out = chunks
					.map(move |result| {
						count += 1;
						if count > MAX_ITEMS {
							return Value::error(err_limit(head, &format!("iteration exceeds {MAX_ITEMS} items")), head);
						}
						result
							.and_then(|value| each_map(value, &mut closure, head))
							.unwrap_or_else(|error| Value::error(error, head))
					})
					.into_pipeline_data(head, engine_state.signals().clone());
				Ok(out)
			}
			PipelineData::Value(value, ..) => ClosureEvalOnce::new(engine_state, stack, closure).run_with_value(value),
		};

		if keep_empty {
			result
		} else {
			result.and_then(|x| x.filter(|v| !v.is_nothing(), engine_state.signals()))
		}
		.map(|data| data.set_metadata(metadata))
	}
}

#[inline]
fn each_map(value: Value, closure: &mut ClosureEval, head: Span) -> Result<Value, ShellError> {
	let span = value.span();
	let is_error = value.is_error();
	closure
		.run_with_value(value)
		.and_then(|pipeline_data| pipeline_data.into_value(head))
		.map_err(|error| chain_error_with_input(error, is_error, span))
}

fn chain_error_with_input(error_source: ShellError, input_is_error: bool, span: Span) -> ShellError {
	if !input_is_error {
		return ShellError::EvalBlockWithInput {
			span,
			sources: vec![error_source],
		};
	}
	error_source
}
