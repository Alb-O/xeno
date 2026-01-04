//! Options configuration parsing.

use kdl::KdlNode;
use xeno_registry::options::{self, OptionStore, OptionType, OptionValue};

use crate::error::{ConfigError, Result};

/// Parse an `options { }` node into an [`OptionStore`].
pub fn parse_options_node(node: &KdlNode) -> Result<OptionStore> {
	parse_options_from_children(node)
}

/// Parse options from a node's children (shared by top-level and per-language).
pub fn parse_options_from_children(node: &KdlNode) -> Result<OptionStore> {
	let mut store = OptionStore::new();

	let Some(children) = node.children() else {
		return Ok(store);
	};

	for opt_node in children.nodes() {
		let kdl_key = opt_node.name().value();

		let def = options::find_by_kdl(kdl_key).ok_or_else(|| ConfigError::UnknownOption {
			key: kdl_key.to_string(),
			suggestion: suggest_option(kdl_key),
		})?;

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

			let _ = store.set_by_kdl(kdl_key, opt_value);
		}
	}

	Ok(store)
}

/// Suggests a similar option KDL key using fuzzy matching.
///
/// Returns `None` if no option is close enough (edit distance > 3).
fn suggest_option(key: &str) -> Option<String> {
	options::all_sorted()
		.map(|o| o.kdl_key)
		.min_by_key(|k| strsim::levenshtein(key, k))
		.filter(|k| strsim::levenshtein(key, k) <= 3)
		.map(|s| s.to_string())
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

	#[test]
	fn test_parse_options() {
		let kdl = r##"
options {
    tab-width 4
    use-tabs #false
    theme "gruvbox"
}
"##;
		let doc: kdl::KdlDocument = kdl.parse().unwrap();
		let opts = parse_options_node(doc.get("options").unwrap()).unwrap();

		assert_eq!(opts.get(keys::tab_width), Some(&OptionValue::Int(4)));
		assert_eq!(opts.get(keys::use_tabs), Some(&OptionValue::Bool(false)));
		assert_eq!(
			opts.get(keys::theme),
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
		let result = parse_options_node(doc.get("options").unwrap());

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
		let result = parse_options_node(doc.get("options").unwrap());

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
		let result = parse_options_node(doc.get("options").unwrap());

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
}
