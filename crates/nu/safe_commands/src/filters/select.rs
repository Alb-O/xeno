use crate::limits::{MAX_COLUMNS, MAX_ITEMS, err_limit};
use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `select` implementation (no SQLite).
#[derive(Clone)]
pub struct Select;

impl Command for Select {
	fn name(&self) -> &str {
		"select"
	}

	fn signature(&self) -> Signature {
		Signature::build("select")
			.input_output_types(vec![
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
				(Type::list(Type::Any), Type::list(Type::Any)),
			])
			.rest("rest", SyntaxShape::CellPath, "The columns to select from the table.")
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Select only these columns or rows from the input. Opposite of `reject`."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["pick", "choose", "projection"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let columns: Vec<CellPath> = call.rest(engine_state, stack, 0)?;

		if columns.len() > MAX_COLUMNS {
			return Err(err_limit(head, &format!("select exceeds {MAX_COLUMNS} columns")));
		}
		if columns.is_empty() {
			return Err(ShellError::GenericError {
				error: "Select requires at least one column".into(),
				msg: "no columns specified".into(),
				span: Some(head),
				help: None,
				inner: vec![],
			});
		}

		let metadata = input.metadata();

		match input {
			PipelineData::Value(Value::Record { val, .. }, ..) => {
				let result = select_from_record(&val, &columns, head)?;
				Ok(Value::record(result, head).into_pipeline_data_with_metadata(metadata))
			}
			PipelineData::Value(Value::List { vals, .. }, ..) => {
				if vals.len() > MAX_ITEMS {
					return Err(err_limit(head, &format!("select input exceeds {MAX_ITEMS} rows")));
				}
				let mut output = Vec::with_capacity(vals.len());
				for value in vals {
					match value {
						Value::Record { val, .. } => {
							output.push(Value::record(select_from_record(&val, &columns, head)?, head));
						}
						other => {
							return Err(ShellError::OnlySupportsThisInputType {
								exp_input_type: "record".into(),
								wrong_type: other.get_type().to_string(),
								dst_span: head,
								src_span: other.span(),
							});
						}
					}
				}
				Ok(Value::list(output, head).into_pipeline_data_with_metadata(metadata))
			}
			PipelineData::ListStream(stream, ..) => {
				let mut count = 0usize;
				let stream = stream.map(move |value| {
					count += 1;
					if count > MAX_ITEMS {
						return Value::error(err_limit(head, &format!("select iteration exceeds {MAX_ITEMS} rows")), head);
					}
					match value {
						Value::Record { val, .. } => match select_from_record(&val, &columns, head) {
							Ok(rec) => Value::record(rec, head),
							Err(e) => Value::error(e, head),
						},
						other => Value::error(
							ShellError::OnlySupportsThisInputType {
								exp_input_type: "record".into(),
								wrong_type: other.get_type().to_string(),
								dst_span: head,
								src_span: other.span(),
							},
							head,
						),
					}
				});
				Ok(PipelineData::list_stream(stream, metadata))
			}
			_ => Err(ShellError::OnlySupportsThisInputType {
				exp_input_type: "record, table, or list".into(),
				wrong_type: input.get_type().to_string(),
				dst_span: head,
				src_span: input.span().unwrap_or(head),
			}),
		}
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Select a column from a record.",
				example: "{a: 1, b: 2, c: 3} | select a c",
				result: Some(Value::test_record(record! {
					"a" => Value::test_int(1),
					"c" => Value::test_int(3),
				})),
			},
			Example {
				description: "Select columns from a table.",
				example: "[[name age]; [Alice 30] [Bob 25]] | select name",
				result: Some(Value::test_list(vec![
					Value::test_record(record! { "name" => Value::test_string("Alice") }),
					Value::test_record(record! { "name" => Value::test_string("Bob") }),
				])),
			},
		]
	}
}

fn select_from_record(record: &Record, columns: &[CellPath], span: Span) -> Result<Record, ShellError> {
	let mut result = Record::new();
	for col in columns {
		// Only support simple single-member string paths for select.
		if col.members.len() == 1 {
			if let xeno_nu_protocol::ast::PathMember::String { val: key, .. } = &col.members[0] {
				if let Some(value) = record.get(key) {
					result.push(key.clone(), value.clone());
				} else {
					result.push(key.clone(), Value::nothing(span));
				}
				continue;
			}
		}
		// For complex cell paths, use follow_cell_path on a record Value.
		let rec_val = Value::record(record.clone(), span);
		let val = rec_val.follow_cell_path(&col.members)?.into_owned();
		// Use the last path member name as key.
		let key = match col.members.last() {
			Some(xeno_nu_protocol::ast::PathMember::String { val, .. }) => val.clone(),
			_ => format!("{col:?}"),
		};
		result.push(key, val);
	}
	Ok(result)
}
