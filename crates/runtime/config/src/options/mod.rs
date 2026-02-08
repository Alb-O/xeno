//! Options configuration parsing.

use kdl::KdlNode;
use xeno_registry::options::{self, OptionScope, OptionStore, OptionType, OptionValue, parse};

use crate::error::{ConfigError, ConfigWarning, Result};

#[cfg(test)]
mod tests;

/// Context for option parsing - indicates where options are being parsed from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ParseContext {
	/// Inside a global `options { }` block.
	Global,
	/// Inside a `language "foo" { }` block.
	Language,
}

/// Result of parsing options, including any non-fatal warnings.
#[derive(Debug)]
pub struct ParsedOptions {
	/// The parsed option store.
	pub store: OptionStore,
	/// Non-fatal warnings encountered during parsing.
	pub warnings: Vec<ConfigWarning>,
}

/// Parses options from a KDL node with scope context validation.
///
/// Global-scoped options (like `theme`) in a language block produce warnings
/// and are skipped. Validation failures emit warnings to stderr rather than
/// failing the parse, allowing partial configuration to load.
///
/// # Errors
///
/// Returns [`ConfigError::UnknownOption`] for unrecognized option keys, or
/// [`ConfigError::OptionTypeMismatch`] when a value doesn't match the expected type.
pub fn parse_options_with_context(node: &KdlNode, context: ParseContext) -> Result<ParsedOptions> {
	let mut store = OptionStore::new();
	let mut warnings = Vec::new();

	let Some(children) = node.children() else {
		return Ok(ParsedOptions { store, warnings });
	};

	for opt_node in children.nodes() {
		let kdl_key = opt_node.name().value();

		let def = options::find(kdl_key).ok_or_else(|| ConfigError::UnknownOption {
			key: kdl_key.to_string(),
			suggestion: parse::suggest_option(kdl_key),
		})?;

		if context == ParseContext::Language && def.scope == OptionScope::Global {
			warnings.push(ConfigWarning::ScopeMismatch {
				option: kdl_key.to_string(),
				found_in: "language block",
				expected: "global options block",
			});
			continue;
		}

		let Some(entry) = opt_node.entries().first() else {
			continue;
		};

		let value = entry.value();
		let opt_value = if let Some(b) = value.as_bool() {
			OptionValue::Bool(b)
		} else if let Some(i) = value.as_integer() {
			OptionValue::Int(i as i64)
		} else if let Some(s) = value.as_string() {
			OptionValue::String(s.to_string())
		} else {
			continue;
		};

		if !opt_value.matches_type(def.value_type) {
			return Err(ConfigError::OptionTypeMismatch {
				option: kdl_key.to_string(),
				expected: option_type_name(def.value_type),
				got: opt_value.type_name(),
			});
		}

		if let Err(e) = options::validate(kdl_key, &opt_value) {
			eprintln!("Warning: {e}");
			continue;
		}

		let _ = store.set_by_kdl(&options::OPTIONS, kdl_key, opt_value);
	}

	Ok(ParsedOptions { store, warnings })
}

/// Returns a human-readable name for an option type.
fn option_type_name(ty: OptionType) -> &'static str {
	match ty {
		OptionType::Bool => "bool",
		OptionType::Int => "int",
		OptionType::String => "string",
	}
}
