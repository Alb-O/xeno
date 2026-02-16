/// Shared sort key extraction and comparison for `sort` and `sort-by`.
use xeno_nu_engine::command_prelude::*;

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum SortKey {
	Bool(bool),
	Int(i64),
	Str(String),
	Null,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyType {
	Bool,
	Int,
	Str,
}

/// Extract a sort key from a value. Returns `(key, concrete_type_if_any)`.
pub(crate) fn key_for_value(v: &Value, head: Span) -> Result<(SortKey, Option<KeyType>), ShellError> {
	match v {
		Value::Int { val, .. } => Ok((SortKey::Int(*val), Some(KeyType::Int))),
		Value::String { val, .. } => Ok((SortKey::Str(val.clone()), Some(KeyType::Str))),
		Value::Bool { val, .. } => Ok((SortKey::Bool(*val), Some(KeyType::Bool))),
		Value::Nothing { .. } => Ok((SortKey::Null, None)),
		Value::Error { error, .. } => Err(*error.clone()),
		other => Err(ShellError::GenericError {
			error: "Unsupported type for sorting".into(),
			msg: format!("cannot sort {}", other.get_type()),
			span: Some(head),
			help: Some("sort supports int, string, bool, and null".into()),
			inner: vec![],
		}),
	}
}

/// Validate that a new key type is consistent with previously seen types.
pub(crate) fn validate_homogeneous(seen: &mut Option<KeyType>, kt: Option<KeyType>, head: Span) -> Result<(), ShellError> {
	let Some(kt) = kt else { return Ok(()) };
	match *seen {
		None => {
			*seen = Some(kt);
			Ok(())
		}
		Some(prev) if prev == kt => Ok(()),
		_ => Err(ShellError::GenericError {
			error: "Mixed types in sort".into(),
			msg: "cannot sort a list with mixed concrete types".into(),
			span: Some(head),
			help: Some("all non-null values must be the same type".into()),
			inner: vec![],
		}),
	}
}

/// Compare two sort keys with explicit null policy.
pub(crate) fn compare_keys(a: &SortKey, b: &SortKey, nulls_first: bool) -> std::cmp::Ordering {
	use std::cmp::Ordering::*;
	match (a, b) {
		(SortKey::Null, SortKey::Null) => Equal,
		(SortKey::Null, _) => {
			if nulls_first {
				Less
			} else {
				Greater
			}
		}
		(_, SortKey::Null) => {
			if nulls_first {
				Greater
			} else {
				Less
			}
		}
		(SortKey::Int(a), SortKey::Int(b)) => a.cmp(b),
		(SortKey::Str(a), SortKey::Str(b)) => a.cmp(b),
		(SortKey::Bool(a), SortKey::Bool(b)) => a.cmp(b),
		// Should not happen after homogeneous validation, but handle gracefully.
		_ => Equal,
	}
}

/// Compare two sort keys with explicit null policy and reverse flag.
///
/// `--reverse` reverses non-null ordering; null placement is controlled
/// only by `--nulls-first`.
pub(crate) fn compare_keys_with_order(a: &SortKey, b: &SortKey, nulls_first: bool, reverse: bool) -> std::cmp::Ordering {
	use std::cmp::Ordering::*;
	match (a, b) {
		(SortKey::Null, SortKey::Null) => Equal,
		(SortKey::Null, _) | (_, SortKey::Null) => compare_keys(a, b, nulls_first),
		_ => {
			let cmp = compare_keys(a, b, nulls_first);
			if reverse { cmp.reverse() } else { cmp }
		}
	}
}

/// Compare two vectors of sort keys lexicographically with reverse support.
pub(crate) fn compare_key_vecs_with_order(a: &[SortKey], b: &[SortKey], nulls_first: bool, reverse: bool) -> std::cmp::Ordering {
	for (ka, kb) in a.iter().zip(b.iter()) {
		let cmp = compare_keys_with_order(ka, kb, nulls_first, reverse);
		if cmp != std::cmp::Ordering::Equal {
			return cmp;
		}
	}
	std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
	use std::cmp::Ordering;

	use super::*;

	#[test]
	fn reverse_keeps_null_last() {
		let cmp = compare_keys_with_order(&SortKey::Null, &SortKey::Int(1), false, true);
		assert_eq!(cmp, Ordering::Greater);
	}

	#[test]
	fn reverse_keeps_null_first() {
		let cmp = compare_keys_with_order(&SortKey::Null, &SortKey::Int(1), true, true);
		assert_eq!(cmp, Ordering::Less);
	}

	#[test]
	fn reverse_flips_concrete() {
		let cmp = compare_keys_with_order(&SortKey::Int(1), &SortKey::Int(2), false, true);
		assert_eq!(cmp, Ordering::Greater);
	}

	#[test]
	fn no_reverse() {
		let cmp = compare_keys_with_order(&SortKey::Int(1), &SortKey::Int(2), false, false);
		assert_eq!(cmp, Ordering::Less);
	}

	#[test]
	fn key_vecs_lexicographic() {
		let a = vec![SortKey::Int(1), SortKey::Str("b".into())];
		let b = vec![SortKey::Int(1), SortKey::Str("a".into())];
		let cmp = compare_key_vecs_with_order(&a, &b, false, false);
		assert_eq!(cmp, Ordering::Greater);
	}
}
