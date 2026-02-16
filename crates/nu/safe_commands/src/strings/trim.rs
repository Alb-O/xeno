use crate::strings::column_apply::{extract_column_names, map_columns};
use xeno_nu_engine::command_prelude::*;

/// Xeno-owned minimal `str trim` implementation (no nu-cmd-base).
#[derive(Clone)]
pub struct StrTrim;

impl Command for StrTrim {
	fn name(&self) -> &str {
		"str trim"
	}

	fn signature(&self) -> Signature {
		Signature::build("str trim")
			.input_output_types(vec![
				(Type::String, Type::String),
				(Type::List(Box::new(Type::String)), Type::List(Box::new(Type::String))),
				(Type::record(), Type::record()),
				(Type::table(), Type::table()),
			])
			.named("char", SyntaxShape::String, "character to trim (default: whitespace)", Some('c'))
			.switch("left", "trims characters only from the beginning of the string", Some('l'))
			.switch("right", "trims characters only from the end of the string", Some('r'))
			.rest("rest", SyntaxShape::CellPath, "For a record or table, columns to trim.")
			.category(Category::Strings)
	}

	fn description(&self) -> &str {
		"Trim whitespace or specific character."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["whitespace", "strip", "lstrip", "rstrip"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let character: Option<Spanned<String>> = call.get_flag(engine_state, stack, "char")?;
		let left = call.has_flag(engine_state, stack, "left")?;
		let right = call.has_flag(engine_state, stack, "right")?;

		let to_trim = match character.as_ref() {
			Some(v) => {
				if v.item.chars().count() > 1 {
					return Err(ShellError::GenericError {
						error: "Trim only works with single character".into(),
						msg: "needs single character".into(),
						span: Some(v.span),
						help: None,
						inner: vec![],
					});
				}
				v.item.chars().next()
			}
			None => None,
		};

		let trim_side = match (left, right) {
			(true, false) => TrimSide::Left,
			(false, true) => TrimSide::Right,
			_ => TrimSide::Both,
		};

		let columns: Vec<CellPath> = call.rest(engine_state, stack, 0)?;
		if !columns.is_empty() {
			let col_names = extract_column_names(&columns, head)?;
			return map_columns(input, &col_names, head, engine_state, move |s| {
				Value::string(trim(s, to_trim, &trim_side), head)
			});
		}

		input.map(
			move |value| match &value {
				Value::String { val, .. } => Value::string(trim(val, to_trim, &trim_side), head),
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
				description: "Trim whitespace.",
				example: "'Nu shell ' | str trim",
				result: Some(Value::test_string("Nu shell")),
			},
			Example {
				description: "Trim a specific character.",
				example: "'=== Nu shell ===' | str trim --char '='",
				result: Some(Value::test_string(" Nu shell ")),
			},
		]
	}
}

enum TrimSide {
	Left,
	Right,
	Both,
}

fn trim(s: &str, char_: Option<char>, trim_side: &TrimSide) -> String {
	let delimiters = match char_ {
		Some(c) => vec![c],
		None => vec![' ', '\x09', '\x0A', '\x0B', '\x0C', '\x0D'],
	};

	match trim_side {
		TrimSide::Left => s.trim_start_matches(&delimiters[..]).to_string(),
		TrimSide::Right => s.trim_end_matches(&delimiters[..]).to_string(),
		TrimSide::Both => s.trim_matches(&delimiters[..]).to_string(),
	}
}
