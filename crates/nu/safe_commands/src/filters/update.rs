use xeno_nu_engine::command_prelude::*;
use xeno_nu_engine::{ClosureEval, ClosureEvalOnce};
use xeno_nu_protocol::ast::PathMember;

use crate::limits::{MAX_ITEMS, err_limit};

#[derive(Clone)]
pub struct Update;

impl Command for Update {
	fn name(&self) -> &str {
		"update"
	}

	fn signature(&self) -> Signature {
		Signature::build("update")
			.input_output_types(vec![
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
			])
			.required("field", SyntaxShape::CellPath, "The name of the column to update.")
			.required(
				"replacement value",
				SyntaxShape::Any,
				"The new value to give the cell(s), or a closure to create the value.",
			)
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Update an existing column to have a new value."
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		update(engine_state, stack, call, input)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![Example {
			description: "Update a column value.",
			example: "{'name': 'nu', 'stars': 5} | update name 'Nushell'",
			result: Some(Value::test_record(record! {
				"name" =>  Value::test_string("Nushell"),
				"stars" => Value::test_int(5),
			})),
		}]
	}
}

fn update(engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
	let head = call.head;
	let cell_path: CellPath = call.req(engine_state, stack, 0)?;
	let replacement: Value = call.req(engine_state, stack, 1)?;

	match input {
		PipelineData::Value(mut value, metadata) => {
			if let Value::Closure { val, .. } = replacement {
				match (cell_path.members.first(), &mut value) {
					(Some(PathMember::String { .. }), Value::List { vals, .. }) => {
						if vals.len() > MAX_ITEMS {
							return Err(err_limit(head, &format!("update input exceeds {MAX_ITEMS} rows")));
						}
						let mut closure = ClosureEval::new(engine_state, stack, *val);
						for val in vals {
							update_value_by_closure(val, &mut closure, head, &cell_path.members, false)?;
						}
					}
					(first, _) => {
						update_single_value_by_closure(
							&mut value,
							ClosureEvalOnce::new(engine_state, stack, *val),
							head,
							&cell_path.members,
							matches!(first, Some(PathMember::Int { .. })),
						)?;
					}
				}
			} else {
				value.update_data_at_cell_path(&cell_path.members, replacement)?;
			}
			Ok(value.into_pipeline_data_with_metadata(metadata))
		}
		PipelineData::ListStream(stream, metadata) => {
			if let Some((&PathMember::Int { val, span: path_span, .. }, path)) = cell_path.members.split_first() {
				let mut stream = stream.into_iter();
				let mut pre_elems = vec![];

				for idx in 0..=val {
					if let Some(v) = stream.next() {
						pre_elems.push(v);
					} else if idx == 0 {
						return Err(ShellError::AccessEmptyContent { span: path_span });
					} else {
						return Err(ShellError::AccessBeyondEnd {
							max_idx: idx - 1,
							span: path_span,
						});
					}
				}

				let value = pre_elems.last_mut().expect("one element");

				if let Value::Closure { val, .. } = replacement {
					update_single_value_by_closure(value, ClosureEvalOnce::new(engine_state, stack, *val), head, path, true)?;
				} else {
					value.update_data_at_cell_path(path, replacement)?;
				}

				Ok(pre_elems
					.into_iter()
					.chain(stream)
					.into_pipeline_data_with_metadata(head, engine_state.signals().clone(), metadata))
			} else if let Value::Closure { val, .. } = replacement {
				let mut closure = ClosureEval::new(engine_state, stack, *val);
				let stream = stream.map(move |mut value| {
					let err = update_value_by_closure(&mut value, &mut closure, head, &cell_path.members, false);
					if let Err(e) = err { Value::error(e, head) } else { value }
				});
				Ok(PipelineData::list_stream(stream, metadata))
			} else {
				let stream = stream.map(move |mut value| {
					if let Err(e) = value.update_data_at_cell_path(&cell_path.members, replacement.clone()) {
						Value::error(e, head)
					} else {
						value
					}
				});
				Ok(PipelineData::list_stream(stream, metadata))
			}
		}
		PipelineData::Empty => Err(ShellError::IncompatiblePathAccess {
			type_name: "empty pipeline".to_string(),
			span: head,
		}),
		PipelineData::ByteStream(stream, ..) => Err(ShellError::IncompatiblePathAccess {
			type_name: stream.type_().describe().into(),
			span: head,
		}),
	}
}

fn update_value_by_closure(
	value: &mut Value,
	closure: &mut ClosureEval,
	span: Span,
	cell_path: &[PathMember],
	first_path_member_int: bool,
) -> Result<(), ShellError> {
	let value_at_path = value.follow_cell_path(cell_path)?;

	let is_optional = cell_path.iter().any(|member| match member {
		PathMember::String { optional, .. } => *optional,
		PathMember::Int { optional, .. } => *optional,
	});
	if is_optional && matches!(value_at_path.as_ref(), Value::Nothing { .. }) {
		return Ok(());
	}

	let arg = if first_path_member_int { value_at_path.as_ref() } else { &*value };

	let new_value = closure
		.add_arg(arg.clone())
		.run_with_input(value_at_path.into_owned().into_pipeline_data())?
		.into_value(span)?;

	value.update_data_at_cell_path(cell_path, new_value)
}

fn update_single_value_by_closure(
	value: &mut Value,
	closure: ClosureEvalOnce,
	span: Span,
	cell_path: &[PathMember],
	first_path_member_int: bool,
) -> Result<(), ShellError> {
	let value_at_path = value.follow_cell_path(cell_path)?;

	let is_optional = cell_path.iter().any(|member| match member {
		PathMember::String { optional, .. } => *optional,
		PathMember::Int { optional, .. } => *optional,
	});
	if is_optional && matches!(value_at_path.as_ref(), Value::Nothing { .. }) {
		return Ok(());
	}

	let arg = if first_path_member_int { value_at_path.as_ref() } else { &*value };

	let new_value = closure
		.add_arg(arg.clone())
		.run_with_input(value_at_path.into_owned().into_pipeline_data())?
		.into_value(span)?;

	value.update_data_at_cell_path(cell_path, new_value)
}
