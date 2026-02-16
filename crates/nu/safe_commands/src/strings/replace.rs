use xeno_nu_engine::command_prelude::*;

use crate::strings::column_apply::{extract_column_names, map_columns};

/// Xeno-owned literal-only `str replace` implementation (no regex).
#[derive(Clone)]
pub struct StrReplace;

impl Command for StrReplace {
	fn name(&self) -> &str {
		"str replace"
	}

	fn signature(&self) -> Signature {
		Signature::build("str replace")
			.input_output_types(vec![
				(Type::String, Type::String),
				(Type::List(Box::new(Type::String)), Type::List(Box::new(Type::String))),
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
			])
			.required("find", SyntaxShape::String, "The pattern to find.")
			.required("replace", SyntaxShape::String, "The replacement string.")
			.switch("all", "Replace all occurrences of the pattern.", Some('a'))
			.switch("regex", "Use regex syntax for the pattern (disabled in xeno sandbox).", Some('r'))
			.rest("rest", SyntaxShape::CellPath, "For a record or table, columns to operate on.")
			.category(Category::Strings)
	}

	fn description(&self) -> &str {
		"Find and replace text."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["substitute", "sed"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let find_str: String = call.req(engine_state, stack, 0)?;
		let replace_str: String = call.req(engine_state, stack, 1)?;
		let replace_all = call.has_flag(engine_state, stack, "all")?;
		let has_regex = call.has_flag(engine_state, stack, "regex")?;

		if has_regex {
			return Err(ShellError::GenericError {
				error: "Regex replace is disabled in xeno sandbox".into(),
				msg: "regex mode not available".into(),
				span: Some(head),
				help: Some("Use literal string patterns instead.".into()),
				inner: vec![],
			});
		}

		let columns: Vec<CellPath> = call.rest(engine_state, stack, 2)?;
		if !columns.is_empty() {
			let col_names = extract_column_names(&columns, head)?;
			return map_columns(input, &col_names, head, engine_state, move |s| {
				let result = if replace_all {
					s.replace(&find_str, &replace_str)
				} else {
					s.replacen(&find_str, &replace_str, 1)
				};
				Value::string(result, head)
			});
		}

		input.map(
			move |value| match &value {
				Value::String { val, .. } => {
					let result = if replace_all {
						val.replace(&find_str, &replace_str)
					} else {
						val.replacen(&find_str, &replace_str, 1)
					};
					Value::string(result, head)
				}
				Value::Error { .. } => value,
				_ => Value::error(
					ShellError::OnlySupportsThisInputType {
						exp_input_type: "string".into(),
						wrong_type: value.get_type().to_string(),
						dst_span: head,
						src_span: value.span(),
					},
					head,
				),
			},
			engine_state.signals(),
		)
	}

	fn examples(&self) -> Vec<Example<'_>> {
		vec![
			Example {
				description: "Find and replace the first occurrence of a substring.",
				example: "'c]test[b' | str replace ']' '}'",
				result: Some(Value::test_string("c}test[b")),
			},
			Example {
				description: "Find and replace all occurrences of a substring.",
				example: "'abc abc' | str replace --all 'b' 'z'",
				result: Some(Value::test_string("azc azc")),
			},
		]
	}
}
