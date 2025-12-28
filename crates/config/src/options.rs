//! Options configuration parsing.

use std::collections::HashMap;

use kdl::KdlNode;

use crate::error::Result;

/// Option value types matching [`evildoer_manifest::OptionValue`].
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
	Bool(bool),
	Int(i64),
	String(String),
}

impl OptionValue {
	pub fn as_bool(&self) -> Option<bool> {
		if let Self::Bool(v) = self {
			Some(*v)
		} else {
			None
		}
	}

	pub fn as_int(&self) -> Option<i64> {
		if let Self::Int(v) = self {
			Some(*v)
		} else {
			None
		}
	}

	pub fn as_str(&self) -> Option<&str> {
		if let Self::String(v) = self {
			Some(v)
		} else {
			None
		}
	}
}

/// Options configuration mapping option names to values.
#[derive(Debug, Clone, Default)]
pub struct OptionsConfig {
	pub values: HashMap<String, OptionValue>,
}

impl OptionsConfig {
	/// Merge another options config, with `other` taking precedence.
	pub fn merge(&mut self, other: OptionsConfig) {
		self.values.extend(other.values);
	}

	pub fn get(&self, name: &str) -> Option<&OptionValue> {
		self.values.get(name)
	}
}

/// Parse an `options { }` node into [`OptionsConfig`].
pub fn parse_options_node(node: &KdlNode) -> Result<OptionsConfig> {
	parse_options_from_children(node)
}

/// Parse options from a node's children (shared by top-level and per-language).
pub fn parse_options_from_children(node: &KdlNode) -> Result<OptionsConfig> {
	let mut config = OptionsConfig::default();

	let Some(children) = node.children() else {
		return Ok(config);
	};

	for opt_node in children.nodes() {
		let name = opt_node.name().value().to_string();

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

			config.values.insert(name, opt_value);
		}
	}

	Ok(config)
}

#[cfg(test)]
mod tests {
	use super::*;

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

		assert_eq!(opts.get("tab-width"), Some(&OptionValue::Int(4)));
		assert_eq!(opts.get("use-tabs"), Some(&OptionValue::Bool(false)));
		assert_eq!(
			opts.get("theme"),
			Some(&OptionValue::String("gruvbox".to_string()))
		);
	}
}
