use crate::limits::{MAX_ITEMS, MAX_SPLITS, err_limit};
use crate::strings::column_apply::extract_column_names;
use xeno_nu_engine::command_prelude::*;

/// Xeno-owned literal-only `split row` implementation (no regex).
#[derive(Clone)]
pub struct SplitRow;

impl Command for SplitRow {
	fn name(&self) -> &str {
		"split row"
	}

	fn signature(&self) -> Signature {
		Signature::build("split row")
			.input_output_types(vec![
				(Type::String, Type::List(Box::new(Type::String))),
				(Type::List(Box::new(Type::String)), Type::List(Box::new(Type::String))),
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
			])
			.required("separator", SyntaxShape::String, "A string that denotes what separates rows.")
			.named("number", SyntaxShape::Int, "Split into maximum number of items", Some('n'))
			.switch("regex", "Use regex syntax for separator (disabled in xeno sandbox).", Some('r'))
			.rest("rest", SyntaxShape::CellPath, "For a record or table, columns to split.")
			.allow_variants_without_examples(true)
			.category(Category::Strings)
	}

	fn description(&self) -> &str {
		"Split a string into multiple rows using a separator."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["separate", "divide"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let separator: String = call.req(engine_state, stack, 0)?;
		let max_split: Option<usize> = call.get_flag(engine_state, stack, "number")?;
		let has_regex = call.has_flag(engine_state, stack, "regex")?;

		if has_regex {
			return Err(ShellError::GenericError {
				error: "Regex split is disabled in xeno sandbox".into(),
				msg: "regex mode not available".into(),
				span: Some(head),
				help: Some("Use literal string separators instead.".into()),
				inner: vec![],
			});
		}

		let columns: Vec<CellPath> = call.rest(engine_state, stack, 1)?;
		if !columns.is_empty() {
			let col_names = extract_column_names(&columns, head)?;
			return split_row_columns(input, &col_names, &separator, max_split, head, engine_state);
		}

		input.flat_map(
			move |value| {
				let parts = split_row_helper(&value, &separator, max_split, head);
				if parts.len() > MAX_SPLITS {
					return vec![Value::error(err_limit(head, &format!("split row exceeds {MAX_SPLITS} segments")), head)];
				}
				parts
			},
			engine_state.signals(),
		)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Split a string into rows by the specified separator.",
				example: "'a--b--c' | split row '--'",
				result: Some(Value::list(
					vec![Value::test_string("a"), Value::test_string("b"), Value::test_string("c")],
					Span::test_data(),
				)),
			},
			Example {
				description: "Split a string by '-'.",
				example: "'-a-b-c-' | split row '-'",
				result: Some(Value::list(
					vec![
						Value::test_string(""),
						Value::test_string("a"),
						Value::test_string("b"),
						Value::test_string("c"),
						Value::test_string(""),
					],
					Span::test_data(),
				)),
			},
		]
	}
}

fn split_row_helper(v: &Value, separator: &str, max_split: Option<usize>, name: Span) -> Vec<Value> {
	let span = v.span();
	match v {
		Value::Error { error, .. } => vec![Value::error(*error.clone(), span)],
		v => {
			if let Ok(s) = v.coerce_str() {
				match max_split {
					Some(n) => s.splitn(n, separator).map(|part| Value::string(part, span)).collect(),
					None => s.split(separator).map(|part| Value::string(part, span)).collect(),
				}
			} else {
				vec![Value::error(
					ShellError::OnlySupportsThisInputType {
						exp_input_type: "string".into(),
						wrong_type: v.get_type().to_string(),
						dst_span: name,
						src_span: span,
					},
					name,
				)]
			}
		}
	}
}

/// Column mode for `split row`: splits string fields into lists of strings.
fn split_row_columns(
	input: PipelineData,
	columns: &[String],
	separator: &str,
	max_split: Option<usize>,
	head: Span,
	engine_state: &EngineState,
) -> Result<PipelineData, ShellError> {
	let columns = columns.to_vec();
	let separator = separator.to_string();
	let metadata = input.metadata();

	let apply = move |record: &mut Record| -> Result<(), ShellError> {
		for col_name in &columns {
			let Some(val) = record.get_mut(col_name) else { continue };
			match val {
				Value::String { val: s, .. } => {
					let parts: Vec<Value> = match max_split {
						Some(n) => s.splitn(n, separator.as_str()).map(|p| Value::string(p, head)).collect(),
						None => s.split(separator.as_str()).map(|p| Value::string(p, head)).collect(),
					};
					if parts.len() > MAX_SPLITS {
						return Err(err_limit(head, &format!("split row exceeds {MAX_SPLITS} segments")));
					}
					*val = Value::list(parts, head);
				}
				Value::Nothing { .. } => {}
				other => {
					return Err(ShellError::OnlySupportsThisInputType {
						exp_input_type: "string".into(),
						wrong_type: other.get_type().to_string(),
						dst_span: head,
						src_span: other.span(),
					});
				}
			}
		}
		Ok(())
	};

	match input {
		PipelineData::Value(Value::Record { val, .. }, ..) => {
			let mut rec = val.into_owned();
			apply(&mut rec)?;
			Ok(Value::record(rec, head).into_pipeline_data_with_metadata(metadata))
		}
		PipelineData::Value(Value::List { vals, .. }, ..) => {
			if vals.len() > MAX_ITEMS {
				return Err(err_limit(head, &format!("input exceeds {MAX_ITEMS} rows")));
			}
			let mut out = Vec::with_capacity(vals.len());
			for value in vals {
				match value {
					Value::Record { val, .. } => {
						let mut rec = val.into_owned();
						apply(&mut rec)?;
						out.push(Value::record(rec, head));
					}
					other => {
						return Err(ShellError::OnlySupportsThisInputType {
							exp_input_type: "record or table".into(),
							wrong_type: other.get_type().to_string(),
							dst_span: head,
							src_span: other.span(),
						});
					}
				}
			}
			Ok(Value::list(out, head).into_pipeline_data_with_metadata(metadata))
		}
		PipelineData::ListStream(stream, ..) => {
			let mut count = 0usize;
			let apply = apply;
			let result = stream
				.into_iter()
				.map(move |value| {
					count += 1;
					if count > MAX_ITEMS {
						return Value::error(err_limit(head, &format!("iteration exceeds {MAX_ITEMS} rows")), head);
					}
					match value {
						Value::Record { val, .. } => {
							let mut rec = val.into_owned();
							if let Err(e) = apply(&mut rec) {
								Value::error(e, head)
							} else {
								Value::record(rec, head)
							}
						}
						other => Value::error(
							ShellError::OnlySupportsThisInputType {
								exp_input_type: "record or table".into(),
								wrong_type: other.get_type().to_string(),
								dst_span: head,
								src_span: other.span(),
							},
							head,
						),
					}
				})
				.into_pipeline_data(head, engine_state.signals().clone());
			Ok(result)
		}
		_ => Err(ShellError::OnlySupportsThisInputType {
			exp_input_type: "record or table".into(),
			wrong_type: input.get_type().to_string(),
			dst_span: head,
			src_span: input.span().unwrap_or(head),
		}),
	}
}
