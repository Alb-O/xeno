//! Standard validators for option values.
//!
//! These functions can be used as validators in [`OptionDef`](crate::OptionDef).

use crate::OptionValue;

/// Validates that an integer is positive (>= 1).
pub fn positive_int(value: &OptionValue) -> Result<(), String> {
	match value {
		OptionValue::Int(n) if *n >= 1 => Ok(()),
		OptionValue::Int(n) => Err(format!("must be at least 1, got {n}")),
		_ => Err("expected integer".to_string()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_positive_int() {
		assert!(positive_int(&OptionValue::Int(1)).is_ok());
		assert!(positive_int(&OptionValue::Int(100)).is_ok());
		assert!(positive_int(&OptionValue::Int(0)).is_err());
		assert!(positive_int(&OptionValue::Int(-1)).is_err());
		assert!(positive_int(&OptionValue::String("foo".into())).is_err());
	}
}
