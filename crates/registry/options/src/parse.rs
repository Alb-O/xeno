//! Shared parsing utilities for option values.
//!
//! This module consolidates all option value parsing logic, used by both
//! config file loading and runtime `:set` commands.

use crate::{OptionError, OptionType, OptionValue, find_by_kdl, all_sorted, validate};

/// Parse a string value into an [`OptionValue`] based on the option's declared type.
///
/// This is the primary entry point for parsing user-provided string values
/// (e.g., from `:set tab-width 4`). It performs both type parsing and custom
/// validation via the option's validator (if defined).
///
/// # Errors
///
/// Returns [`OptionError::UnknownOption`] if the KDL key is not recognized.
/// Returns [`OptionError::InvalidValue`] if the value cannot be parsed as the expected type
/// or fails custom validation.
pub fn parse_value(kdl_key: &str, value: &str) -> Result<OptionValue, OptionError> {
	let def =
		find_by_kdl(kdl_key).ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
	let opt_value =
		parse_value_for_type(value, def.value_type).map_err(|reason| OptionError::InvalidValue {
			option: kdl_key.to_string(),
			reason,
		})?;

	// Run type checking and custom validator
	validate(kdl_key, &opt_value)?;

	Ok(opt_value)
}

/// Parse a string value into an [`OptionValue`] for a known type.
///
/// This is useful when the option type is already known (e.g., from a definition).
///
/// # Errors
///
/// Returns a human-readable error message if parsing fails.
pub fn parse_value_for_type(value: &str, ty: OptionType) -> Result<OptionValue, String> {
	match ty {
		OptionType::Bool => parse_bool(value).map(OptionValue::Bool),
		OptionType::Int => parse_int(value).map(OptionValue::Int),
		OptionType::String => Ok(OptionValue::String(value.to_string())),
	}
}

/// Parse a boolean value from common string representations.
///
/// Accepts: `true`, `1`, `yes`, `on` (case-insensitive) for true
/// Accepts: `false`, `0`, `no`, `off` (case-insensitive) for false
pub fn parse_bool(value: &str) -> Result<bool, String> {
	match value.to_lowercase().as_str() {
		"true" | "1" | "yes" | "on" => Ok(true),
		"false" | "0" | "no" | "off" => Ok(false),
		_ => Err(format!("invalid boolean: '{value}' (expected true/false, yes/no, on/off, 1/0)")),
	}
}

/// Parse an integer value.
pub fn parse_int(value: &str) -> Result<i64, String> {
	value
		.parse::<i64>()
		.map_err(|_| format!("invalid integer: '{value}'"))
}

/// Suggests a similar option KDL key using fuzzy matching.
///
/// Returns `None` if no option is close enough (edit distance > 3).
///
/// # Example
///
/// ```ignore
/// let suggestion = suggest_option("tab-wdith"); // Some("tab-width")
/// let suggestion = suggest_option("xyzabc");    // None
/// ```
pub fn suggest_option(key: &str) -> Option<String> {
	// First check if this is a deprecated option
	if let Some(msg) = deprecated_option_message(key) {
		return Some(msg);
	}

	all_sorted()
		.map(|o| o.kdl_key)
		.min_by_key(|k| strsim::levenshtein(key, k))
		.filter(|k| strsim::levenshtein(key, k) <= 3)
		.map(|s| s.to_string())
}

/// Options that were removed (had no implementation).
const REMOVED_OPTIONS: &[&str] = &[
	"indent-width", "use-tabs", "line-numbers", "wrap-lines", "cursorline",
	"cursorcolumn", "colorcolumn", "whitespace-visible", "scroll-margin",
	"scroll-smooth", "backup", "undo-file", "auto-save", "final-newline",
	"trim-trailing-whitespace", "search-case-sensitive", "search-smart-case",
	"search-wrap", "incremental-search", "mouse", "line-ending", "idle-timeout",
];

/// Returns a deprecation message for removed options.
pub fn deprecated_option_message(key: &str) -> Option<String> {
	REMOVED_OPTIONS
		.contains(&key)
		.then(|| format!("'{key}' was removed (not yet implemented)"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_bool() {
		assert_eq!(parse_bool("true"), Ok(true));
		assert_eq!(parse_bool("TRUE"), Ok(true));
		assert_eq!(parse_bool("yes"), Ok(true));
		assert_eq!(parse_bool("1"), Ok(true));
		assert_eq!(parse_bool("on"), Ok(true));

		assert_eq!(parse_bool("false"), Ok(false));
		assert_eq!(parse_bool("FALSE"), Ok(false));
		assert_eq!(parse_bool("no"), Ok(false));
		assert_eq!(parse_bool("0"), Ok(false));
		assert_eq!(parse_bool("off"), Ok(false));

		assert!(parse_bool("maybe").is_err());
	}

	#[test]
	fn test_parse_int() {
		assert_eq!(parse_int("42"), Ok(42));
		assert_eq!(parse_int("-10"), Ok(-10));
		assert_eq!(parse_int("0"), Ok(0));
		assert!(parse_int("abc").is_err());
		assert!(parse_int("3.14").is_err());
	}

	#[test]
	fn test_parse_value_for_type() {
		assert_eq!(
			parse_value_for_type("true", OptionType::Bool),
			Ok(OptionValue::Bool(true))
		);
		assert_eq!(
			parse_value_for_type("42", OptionType::Int),
			Ok(OptionValue::Int(42))
		);
		assert_eq!(
			parse_value_for_type("hello", OptionType::String),
			Ok(OptionValue::String("hello".to_string()))
		);
	}
}
