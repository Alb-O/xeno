use crate::limits::{MAX_ITEMS, err_limit};
use xeno_nu_engine::{ClosureEval, command_prelude::*};
use xeno_nu_protocol::engine::Closure;

#[derive(Clone)]
pub struct Reduce;

impl Command for Reduce {
	fn name(&self) -> &str {
		"reduce"
	}

	fn signature(&self) -> Signature {
		Signature::build("reduce")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::Any),
				(Type::table(), Type::Any),
				(Type::Range, Type::Any),
			])
			.named("fold", SyntaxShape::Any, "reduce with initial value", Some('f'))
			.required(
				"closure",
				SyntaxShape::Closure(Some(vec![SyntaxShape::Any, SyntaxShape::Any])),
				"Reducing function.",
			)
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Aggregate a list (starting from the left) to a single value using an accumulator closure."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["map", "fold", "foldl"]
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				example: "[ 1 2 3 4 ] | reduce {|it, acc| $it + $acc }",
				description: "Sum values of a list (same as 'math sum').",
				result: Some(Value::test_int(10)),
			},
			Example {
				example: "[ 1 2 3 4 ] | reduce --fold 10 {|it, acc| $acc + $it }",
				description: "Sum values with a starting value (fold).",
				result: Some(Value::test_int(20)),
			},
		]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let fold: Option<Value> = call.get_flag(engine_state, stack, "fold")?;
		let closure: Closure = call.req(engine_state, stack, 0)?;

		let mut iter = input.into_iter();

		let mut acc = fold.or_else(|| iter.next()).ok_or_else(|| ShellError::GenericError {
			error: "Expected input".into(),
			msg: "needs input".into(),
			span: Some(head),
			help: None,
			inner: vec![],
		})?;

		let mut closure = ClosureEval::new(engine_state, stack, closure);
		let mut count = 0usize;

		for value in iter {
			count += 1;
			if count > MAX_ITEMS {
				return Err(err_limit(head, &format!("reduce iteration exceeds {MAX_ITEMS} items")));
			}
			engine_state.signals().check(&head)?;
			acc = closure
				.add_arg(value)
				.add_arg(acc.clone())
				.run_with_input(PipelineData::value(acc, None))?
				.into_value(head)?;
		}

		Ok(acc.with_span(head).into_pipeline_data())
	}
}
