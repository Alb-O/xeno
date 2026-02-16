use crate::filters::sort_key::{SortKey, compare_key_vecs_with_order, key_for_value, validate_homogeneous};
use crate::limits::collect_list_capped;
use crate::strings::column_apply::extract_column_names;
use xeno_nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct SortBy;

impl Command for SortBy {
	fn name(&self) -> &str {
		"sort-by"
	}

	fn signature(&self) -> Signature {
		Signature::build("sort-by")
			.input_output_types(vec![
				(Type::table(), Type::table()),
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
			])
			.rest("columns", SyntaxShape::CellPath, "Column(s) to sort by.")
			.switch("reverse", "Sort in reverse order.", Some('r'))
			.switch("nulls-first", "Sort null values before concrete values.", None)
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Sort a table by column values. --reverse reverses non-null ordering; null placement controlled only by --nulls-first."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["order", "arrange"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let columns: Vec<CellPath> = call.rest(engine_state, stack, 0)?;
		let reverse = call.has_flag(engine_state, stack, "reverse")?;
		let nulls_first = call.has_flag(engine_state, stack, "nulls-first")?;

		if columns.is_empty() {
			return Err(ShellError::GenericError {
				error: "sort-by requires at least one column".into(),
				msg: "no columns specified".into(),
				span: Some(head),
				help: None,
				inner: vec![],
			});
		}

		let col_names = extract_column_names(&columns, head)?;
		let mut vals = collect_list_capped(input, head)?;

		// Per-column type validation: each column must have homogeneous concrete types.
		let mut seen_types: Vec<Option<crate::filters::sort_key::KeyType>> = vec![None; col_names.len()];

		let row_keys: Vec<Vec<SortKey>> = vals
			.iter()
			.map(|v| {
				let rec = v.as_record().map_err(|_| ShellError::OnlySupportsThisInputType {
					exp_input_type: "record".into(),
					wrong_type: v.get_type().to_string(),
					dst_span: head,
					src_span: v.span(),
				})?;

				let mut keys = Vec::with_capacity(col_names.len());
				for (i, col) in col_names.iter().enumerate() {
					let nothing = Value::nothing(head);
					let val = rec.get(col).unwrap_or(&nothing);
					let (key, kt) = key_for_value(val, head)?;
					validate_homogeneous(&mut seen_types[i], kt, head)?;
					keys.push(key);
				}
				Ok(keys)
			})
			.collect::<Result<_, ShellError>>()?;

		let mut indices: Vec<usize> = (0..vals.len()).collect();
		indices.sort_by(|&a, &b| compare_key_vecs_with_order(&row_keys[a], &row_keys[b], nulls_first, reverse));

		let sorted: Vec<Value> = indices.into_iter().map(|i| std::mem::replace(&mut vals[i], Value::nothing(head))).collect();
		Ok(Value::list(sorted, head).into_pipeline_data())
	}
}
