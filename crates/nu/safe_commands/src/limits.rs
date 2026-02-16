use xeno_nu_protocol::{PipelineData, ShellError, Span, Value};

/// Maximum items processed through list/table/stream iteration commands.
pub(crate) const MAX_ITEMS: usize = 10_000;

/// Maximum segments produced by `split row`.
pub(crate) const MAX_SPLITS: usize = 10_000;

/// Maximum column paths for projection/selection commands.
pub(crate) const MAX_COLUMNS: usize = 128;

/// Collect pipeline input into a `Vec<Value>`, stopping at `MAX_ITEMS`.
///
/// Fast path for `Value::List`; streaming path counts items one-by-one.
pub(crate) fn collect_list_capped(input: PipelineData, head: Span) -> Result<Vec<Value>, ShellError> {
	match input {
		PipelineData::Value(Value::List { vals, .. }, ..) => {
			if vals.len() > MAX_ITEMS {
				return Err(err_limit(head, &format!("input list length {} exceeds {MAX_ITEMS}", vals.len())));
			}
			Ok(vals)
		}
		other => {
			let mut out = Vec::new();
			for item in other.into_iter() {
				out.push(item);
				if out.len() > MAX_ITEMS {
					return Err(err_limit(head, &format!("input exceeds {MAX_ITEMS} items")));
				}
			}
			Ok(out)
		}
	}
}

pub(crate) fn err_limit(head: Span, msg: &str) -> ShellError {
	ShellError::GenericError {
		error: "xeno sandbox limit exceeded".into(),
		msg: msg.to_string(),
		span: Some(head),
		help: None,
		inner: vec![],
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn collect_list_capped_fast_path_at_max() {
		let head = Span::unknown();
		let vals: Vec<Value> = (0..MAX_ITEMS).map(|i| Value::int(i as i64, head)).collect();
		let input = PipelineData::Value(Value::list(vals, head), None);
		let result = collect_list_capped(input, head).expect("should pass at MAX_ITEMS");
		assert_eq!(result.len(), MAX_ITEMS);
	}

	#[test]
	fn collect_list_capped_fast_path_over_max() {
		let head = Span::unknown();
		let vals: Vec<Value> = (0..=MAX_ITEMS).map(|i| Value::int(i as i64, head)).collect();
		let input = PipelineData::Value(Value::list(vals, head), None);
		let err = collect_list_capped(input, head).expect_err("should reject MAX_ITEMS+1");
		assert!(format!("{err}").contains("limit") || format!("{err}").contains("exceeds"), "got: {err}");
	}
}
