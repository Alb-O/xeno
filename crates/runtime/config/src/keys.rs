//! Keybinding configuration parsing.

use std::collections::HashMap;

use kdl::KdlNode;

use crate::error::Result;

/// Keybinding configuration mapping modes to key-action pairs.
#[derive(Debug, Clone, Default)]
pub struct KeysConfig {
	/// Bindings per mode. Key: mode name, Value: key string -> action name.
	pub modes: HashMap<String, HashMap<String, String>>,
}

impl KeysConfig {
	/// Merge another keys config, with `other` taking precedence.
	pub fn merge(&mut self, other: KeysConfig) {
		for (mode, bindings) in other.modes {
			self.modes.entry(mode).or_default().extend(bindings);
		}
	}
}

/// Parse a `keys { }` node into [`KeysConfig`].
pub fn parse_keys_node(node: &KdlNode) -> Result<KeysConfig> {
	let mut config = KeysConfig::default();

	let Some(children) = node.children() else {
		return Ok(config);
	};

	for mode_node in children.nodes() {
		let mode_name = mode_node.name().value().to_string();
		let mut bindings = HashMap::new();

		if let Some(mode_children) = mode_node.children() {
			for binding_node in mode_children.nodes() {
				let key = binding_node.name().value().to_string();
				if let Some(action) = binding_node.get(0).and_then(|v| v.as_string()) {
					bindings.insert(key, action.to_string());
				}
			}
		}

		config.modes.insert(mode_name, bindings);
	}

	Ok(config)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_keys() {
		let kdl = r##"
keys {
    normal {
        "ctrl+s" "write"
        "ctrl+q" "quit"
    }
    insert {
        "ctrl+c" "normal_mode"
    }
}
"##;
		let doc: kdl::KdlDocument = kdl.parse().unwrap();
		let keys = parse_keys_node(doc.get("keys").unwrap()).unwrap();

		assert_eq!(
			keys.modes.get("normal").unwrap().get("ctrl+s"),
			Some(&"write".to_string())
		);
		assert_eq!(
			keys.modes.get("insert").unwrap().get("ctrl+c"),
			Some(&"normal_mode".to_string())
		);
	}
}
