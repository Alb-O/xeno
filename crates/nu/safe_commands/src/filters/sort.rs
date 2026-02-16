use crate::filters::sort_key::{SortKey, compare_keys_with_order, key_for_value, validate_homogeneous};
use crate::limits::collect_list_capped;
use xeno_nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct Sort;

impl Command for Sort {
	fn name(&self) -> &str {
		"sort"
	}

	fn signature(&self) -> Signature {
		Signature::build("sort")
			.input_output_types(vec![(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any)))])
			.switch("reverse", "Sort in reverse order.", Some('r'))
			.switch("nulls-first", "Sort null values before concrete values.", None)
			.allow_variants_without_examples(true)
			.category(Category::Filters)
	}

	fn description(&self) -> &str {
		"Sort a list of values. --reverse reverses non-null ordering; null placement controlled only by --nulls-first."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["order", "arrange"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let head = call.head;
		let reverse = call.has_flag(engine_state, stack, "reverse")?;
		let nulls_first = call.has_flag(engine_state, stack, "nulls-first")?;
		let mut vals = collect_list_capped(input, head)?;

		let mut seen_type = None;
		let keys: Vec<SortKey> = vals
			.iter()
			.map(|v| {
				let (key, kt) = key_for_value(v, head)?;
				validate_homogeneous(&mut seen_type, kt, head)?;
				Ok(key)
			})
			.collect::<Result<_, ShellError>>()?;

		let mut indices: Vec<usize> = (0..vals.len()).collect();
		indices.sort_by(|&a, &b| compare_keys_with_order(&keys[a], &keys[b], nulls_first, reverse));

		let sorted: Vec<Value> = indices.into_iter().map(|i| std::mem::replace(&mut vals[i], Value::nothing(head))).collect();
		Ok(Value::list(sorted, head).into_pipeline_data())
	}
}
