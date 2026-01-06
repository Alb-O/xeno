//! Options configuration parsing.

use kdl::KdlNode;
use xeno_registry::options::{self, OptionScope, OptionStore, OptionType, OptionValue, parse};

use crate::error::{ConfigError, ConfigWarning, Result};

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



/// Parse options from a node with scope context validation.
///
/// Returns both the parsed options and any warnings about scope mismatches.
pub fn parse_options_with_context(node: &KdlNode, context: ParseContext) -> Result<ParsedOptions> {
	let mut store = OptionStore::new();
	let mut warnings = Vec::new();

	let Some(children) = node.children() else {
		return Ok(ParsedOptions { store, warnings });
	};

	for opt_node in children.nodes() {
		let kdl_key = opt_node.name().value();

		let def = options::find_by_kdl(kdl_key).ok_or_else(|| ConfigError::UnknownOption {
			key: kdl_key.to_string(),
			suggestion: parse::suggest_option(kdl_key),
		})?;

		// Check for scope mismatches
		if context == ParseContext::Language && def.scope == OptionScope::Global {
			warnings.push(ConfigWarning::ScopeMismatch {
				option: kdl_key.to_string(),
				found_in: "language block",
				expected: "global options block",
			});
			continue; // Skip this option, it won't work in language scope
		}

		if let Some(entry) = opt_node.entries().first() {
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

			// Run custom validator if defined (emit warning on failure, don't fail)
			if let Err(e) = options::validate(kdl_key, &opt_value) {
				eprintln!("Warning: {e}");
				continue; // Skip setting this option
			}

			let _ = store.set_by_kdl(kdl_key, opt_value);
		}
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

#[cfg(test)]
mod tests {
	use super::*;
	use xeno_registry::options::keys;

	fn parse_global(node: &KdlNode) -> Result<ParsedOptions> {
		parse_options_with_context(node, ParseContext::Global)
	}

	#[test]
	fn test_parse_options() {
		let kdl = r##"
options {
    tab-width 4
    theme "gruvbox"
}
"##;
		let doc: kdl::KdlDocument = kdl.parse().unwrap();
		let opts = parse_global(doc.get("options").unwrap()).unwrap().store;

		assert_eq!(opts.get(keys::TAB_WIDTH.untyped()), Some(&OptionValue::Int(4)));
		assert_eq!(
			opts.get(keys::THEME.untyped()),
			Some(&OptionValue::String("gruvbox".to_string()))
		);
	}

	#[test]
	fn test_unknown_option_error() {
		let kdl = r##"
options {
    unknown-option 42
}
"##;
		let doc: kdl::KdlDocument = kdl.parse().unwrap();
		let result = parse_global(doc.get("options").unwrap());

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, ConfigError::UnknownOption { .. }));
	}

	#[test]
	fn test_unknown_option_with_suggestion() {
		let kdl = r##"
options {
    tab-wdith 4
}
"##;
		let doc: kdl::KdlDocument = kdl.parse().unwrap();
		let result = parse_global(doc.get("options").unwrap());

		assert!(result.is_err());
		if let Err(ConfigError::UnknownOption { key, suggestion }) = result {
			assert_eq!(key, "tab-wdith");
			assert_eq!(suggestion, Some("tab-width".to_string()));
		} else {
			panic!("expected UnknownOption error");
		}
	}

	#[test]
	fn test_type_mismatch_error() {
		let kdl = r##"
options {
    tab-width "four"
}
"##;
		let doc: kdl::KdlDocument = kdl.parse().unwrap();
		let result = parse_global(doc.get("options").unwrap());

		assert!(result.is_err());
		if let Err(ConfigError::OptionTypeMismatch {
			option,
			expected,
			got,
		}) = result
		{
			assert_eq!(option, "tab-width");
			assert_eq!(expected, "int");
			assert_eq!(got, "string");
		} else {
			panic!("expected OptionTypeMismatch error");
		}
	}

	#[test]
	fn test_language_specific_options() {
		use crate::Config;

		let kdl = r##"
options {
    tab-width 4
}

language "rust" {
    tab-width 2
}

language "python" {
    tab-width 8
}
"##;
		let config = Config::parse(kdl).unwrap();

		// Global options
		assert_eq!(config.options.get(keys::TAB_WIDTH.untyped()), Some(&OptionValue::Int(4)));

		// Language-specific options
		assert_eq!(config.languages.len(), 2);

		let rust = config.languages.iter().find(|l| l.name == "rust").unwrap();
		assert_eq!(rust.options.get(keys::TAB_WIDTH.untyped()), Some(&OptionValue::Int(2)));

		let python = config.languages.iter().find(|l| l.name == "python").unwrap();
		assert_eq!(python.options.get(keys::TAB_WIDTH.untyped()), Some(&OptionValue::Int(8)));
	}

	#[test]
	fn test_global_option_in_language_block_warns() {
		use crate::error::ConfigWarning;
		use crate::Config;

		let kdl = r##"
language "rust" {
    theme "gruvbox"
}
"##;
		let config = Config::parse(kdl).unwrap();

		// Should have a warning about theme in language block
		assert!(!config.warnings.is_empty(), "expected warnings, got none");
		assert!(
			matches!(
				&config.warnings[0],
				ConfigWarning::ScopeMismatch { option, .. } if option == "theme"
			),
			"expected ScopeMismatch warning for 'theme', got: {:?}",
			config.warnings
		);

		// The option should NOT be set in the language store
		let rust = config.languages.iter().find(|l| l.name == "rust").unwrap();
		assert_eq!(
			rust.options.get(keys::THEME.untyped()),
			None,
			"global option should not be stored in language scope"
		);
	}

	#[test]
	fn test_buffer_scoped_option_in_language_block_ok() {
		use crate::Config;

		let kdl = r##"
language "rust" {
    tab-width 2
}
"##;
		let config = Config::parse(kdl).unwrap();

		// Should have no warnings - tab-width is buffer-scoped
		assert!(
			config.warnings.is_empty(),
			"unexpected warnings: {:?}",
			config.warnings
		);

		// The option should be set
		let rust = config.languages.iter().find(|l| l.name == "rust").unwrap();
		assert_eq!(rust.options.get(keys::TAB_WIDTH.untyped()), Some(&OptionValue::Int(2)));
	}
}
