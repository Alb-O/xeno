//! Standard validators for option values.
//!
//! These functions can be used as validators in [`OptionDef`](crate::options::OptionDef).

use crate::options::OptionValue;

/// Validates that an integer is positive (>= 1).
pub fn positive_int(value: &OptionValue) -> Result<(), String> {
	match value {
		OptionValue::Int(n) if *n >= 1 => Ok(()),
		OptionValue::Int(n) => Err(format!("must be at least 1, got {n}")),
		_ => Err("expected integer".to_string()),
	}
}

#[cfg(test)]
mod tests;
