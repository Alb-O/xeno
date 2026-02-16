use xeno_nu_engine::command_prelude::*;

/// Shared helper for applying string transforms to record columns.
///
/// Used by `str trim`, `str replace`, and `split row` in column mode.
/// Only simple single-member string cell paths are supported.
use crate::limits::{MAX_COLUMNS, MAX_ITEMS, err_limit};

/// Extract simple column names from cell paths. Errors on complex paths or too many columns.
pub(crate) fn extract_column_names(columns: &[CellPath], head: Span) -> Result<Vec<String>, ShellError> {
	if columns.len() > MAX_COLUMNS {
		return Err(err_limit(head, &format!("exceeds {MAX_COLUMNS} columns")));
	}
	let mut names = Vec::with_capacity(columns.len());
	for col in columns {
		if col.members.len() != 1 {
			return Err(ShellError::GenericError {
				error: "Complex cell paths disabled in xeno sandbox".into(),
				msg: "only simple column names are supported".into(),
				span: Some(head),
				help: None,
				inner: vec![],
			});
		}
		match &col.members[0] {
			xeno_nu_protocol::ast::PathMember::String { val, .. } => names.push(val.clone()),
			_ => {
				return Err(ShellError::GenericError {
					error: "Complex cell paths disabled in xeno sandbox".into(),
					msg: "only simple column names are supported".into(),
					span: Some(head),
					help: None,
					inner: vec![],
				});
			}
		}
	}
	Ok(names)
}

/// Apply a string transform `f` to the named columns of a record in-place.
///
/// Missing keys are left as-is. Nothing values are left as-is. Non-string values produce an error.
pub(crate) fn apply_to_record_columns(record: &mut Record, columns: &[String], head: Span, f: &impl Fn(&str) -> Value) -> Result<(), ShellError> {
	for col_name in columns {
		let Some(val) = record.get_mut(col_name) else { continue };
		match val {
			Value::String { val: s, .. } => {
				*val = f(s);
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
}

/// Map a string transform over table/record input using column mode.
///
/// If input is a single record, applies to that record.
/// If input is a list/stream of records, applies to each row with MAX_ITEMS cap.
pub(crate) fn map_columns(
	input: PipelineData,
	columns: &[String],
	head: Span,
	engine_state: &EngineState,
	f: impl Fn(&str) -> Value + Send + Sync + 'static,
) -> Result<PipelineData, ShellError> {
	let metadata = input.metadata();
	let columns = columns.to_vec();

	match input {
		PipelineData::Value(Value::Record { val, .. }, ..) => {
			let mut rec = val.into_owned();
			apply_to_record_columns(&mut rec, &columns, head, &f)?;
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
						apply_to_record_columns(&mut rec, &columns, head, &f)?;
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
							if let Err(e) = apply_to_record_columns(&mut rec, &columns, head, &f) {
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

/// Apply a value transform `f` to the named columns of a record in-place.
///
/// Missing keys and Nothing values are left as-is. The transform receives the
/// full `&Value` (not just string), so it can handle type-specific logic.
pub(crate) fn apply_value_to_record_columns(
	record: &mut Record,
	columns: &[String],
	head: Span,
	f: &impl Fn(&Value) -> Result<Value, ShellError>,
) -> Result<(), ShellError> {
	for col_name in columns {
		let Some(val) = record.get_mut(col_name) else { continue };
		if matches!(val, Value::Nothing { .. }) {
			continue;
		}
		*val = f(val).map_err(|e| ShellError::GenericError {
			error: e.to_string(),
			msg: format!("failed on column '{col_name}'"),
			span: Some(head),
			help: None,
			inner: vec![e],
		})?;
	}
	Ok(())
}

/// Map a value transform over table/record input using column mode.
///
/// Like `map_columns` but accepts `Fn(&Value) -> Result<Value, ShellError>` for
/// type conversions. Enforces MAX_ITEMS on lists/streams.
pub(crate) fn map_columns_values(
	input: PipelineData,
	columns: &[String],
	head: Span,
	engine_state: &EngineState,
	f: impl Fn(&Value) -> Result<Value, ShellError> + Send + Sync + 'static,
) -> Result<PipelineData, ShellError> {
	let metadata = input.metadata();
	let columns = columns.to_vec();

	match input {
		PipelineData::Value(Value::Record { val, .. }, ..) => {
			let mut rec = val.into_owned();
			apply_value_to_record_columns(&mut rec, &columns, head, &f)?;
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
						apply_value_to_record_columns(&mut rec, &columns, head, &f)?;
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
							if let Err(e) = apply_value_to_record_columns(&mut rec, &columns, head, &f) {
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

#[cfg(test)]
mod tests {
	use xeno_nu_protocol::ast::{CellPath, PathMember};
	use xeno_nu_protocol::casing::Casing;

	use super::*;

	fn simple_cell_path(name: &str) -> CellPath {
		CellPath {
			members: vec![PathMember::test_string(name.into(), false, Casing::Sensitive)],
		}
	}

	#[test]
	fn extract_simple_columns() {
		let paths = vec![simple_cell_path("foo"), simple_cell_path("bar")];
		let names = extract_column_names(&paths, Span::unknown()).expect("should succeed");
		assert_eq!(names, vec!["foo", "bar"]);
	}

	#[test]
	fn extract_rejects_multi_member() {
		let path = CellPath {
			members: vec![
				PathMember::test_string("a".into(), false, Casing::Sensitive),
				PathMember::test_string("b".into(), false, Casing::Sensitive),
			],
		};
		let err = extract_column_names(&[path], Span::unknown()).expect_err("should reject");
		let msg = format!("{err}");
		assert!(msg.contains("Complex") || msg.contains("cell path") || msg.contains("disabled"), "got: {msg}");
	}
}
