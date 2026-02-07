//! Shared parsing utilities for option values.
//!
//! This module consolidates all option value parsing logic, used by both
//! config file loading and runtime `:set` commands.

use crate::options::{OptionError, OptionType, OptionValue};

/// Parse a string value into an [`OptionValue`] based on the option's declared type.
pub fn parse_value(kdl_key: &str, value: &str) -> Result<OptionValue, OptionError> {
	let entry = crate::options::OPTIONS
		.get(kdl_key)
		.ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;

	let opt_value = parse_value_for_type(value, entry.value_type).map_err(|reason| {
		OptionError::InvalidValue {
			option: kdl_key.to_string(),
			reason,
		}
	})?;

	if !opt_value.matches_type(entry.value_type) {
		return Err(OptionError::TypeMismatch {
			option: kdl_key.to_string(),
			expected: entry.value_type,
			got: opt_value.type_name(),
		});
	}
	if let Some(validator) = entry.validator {
		validator(&opt_value).map_err(|reason| OptionError::InvalidValue {
			option: kdl_key.to_string(),
			reason,
		})?;
	}

	Ok(opt_value)
}

/// Parse a string value into an [`OptionValue`] for a known type.
pub fn parse_value_for_type(value: &str, ty: OptionType) -> Result<OptionValue, String> {
	match ty {
		OptionType::Bool => parse_bool(value).map(OptionValue::Bool),
		OptionType::Int => parse_int(value).map(OptionValue::Int),
		OptionType::String => Ok(OptionValue::String(value.to_string())),
	}
}

/// Parse a boolean value from common string representations.
pub fn parse_bool(value: &str) -> Result<bool, String> {
	match value.to_lowercase().as_str() {
		"true" | "1" | "yes" | "on" => Ok(true),
		"false" | "0" | "no" | "off" => Ok(false),
		_ => Err(format!(
			"invalid boolean: '{value}' (expected true/false, yes/no, on/off, 1/0)"
		)),
	}
}

/// Parse an integer value.
pub fn parse_int(value: &str) -> Result<i64, String> {
	value
		.parse::<i64>()
		.map_err(|_| format!("invalid integer: '{value}'"))
}

/// Suggests a similar option KDL key using fuzzy matching.
pub fn suggest_option(key: &str) -> Option<String> {
	if let Some(msg) = deprecated_option_message(key) {
		return Some(msg);
	}

	crate::options::OPTIONS
		.all()
		.into_iter()
		.map(|o| o.resolve(o.kdl_key).to_string())
		.min_by_key(|k| strsim::levenshtein(key, k))
		.filter(|k| strsim::levenshtein(key, k) <= 3)
}

/// Options that were removed (had no implementation).
const REMOVED_OPTIONS: &[&str] = &[
	"indent-width",
	"use-tabs",
	"line-numbers",
	"wrap-lines",
	"cursorline",
	"cursorcolumn",
	"colorcolumn",
	"whitespace-visible",
	"scroll-margin",
	"scroll-smooth",
	"backup",
	"undo-file",
	"auto-save",
	"final-newline",
	"trim-trailing-whitespace",
	"search-case-sensitive",
	"search-smart-case",
	"search-wrap",
	"incremental-search",
	"mouse",
	"line-ending",
	"idle-timeout",
];

/// Returns a deprecation message for removed options.
pub fn deprecated_option_message(key: &str) -> Option<String> {
	REMOVED_OPTIONS
		.contains(&key)
		.then(|| format!("'{key}' was removed (not yet implemented)"))
}
