//! KDL parsing support for keymap configurations.
//!
//! Provides parsing of keybindings from KDL format using an idiomatic node-based syntax:
//!
//! ```kdl
//! keybindings {
//!     Quit "q" "esc"
//!     Save "ctrl-s" description="Save the file"
//!     GotoLine "g" "l" description="Jump to line number"
//! }
//! ```
//!
//! Each node name is the action, arguments are key bindings, and the optional
//! `description` property provides documentation.

use std::str::FromStr;

use kdl::{KdlDocument, KdlNode};

use crate::config::{Config, Item};

/// Error type for KDL parsing failures.
#[derive(Debug)]
pub enum Error {
	/// KDL syntax error
	Parse(kdl::KdlError),
	/// Missing required node
	MissingNode(String),
	/// Invalid key binding syntax
	InvalidKey {
		/// Name of the action the binding was for.
		action: String,
		/// The key string that failed to parse.
		key: String,
		/// Description of why parsing failed.
		reason: String,
	},
	/// Action variant not recognized
	UnknownAction(String),
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::Parse(e) => write!(f, "KDL parse error: {e}"),
			Error::MissingNode(name) => write!(f, "missing required node: {name}"),
			Error::InvalidKey {
				action,
				key,
				reason,
			} => {
				write!(f, "invalid key '{key}' for action '{action}': {reason}")
			}
			Error::UnknownAction(name) => write!(f, "unknown action: {name}"),
		}
	}
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Error::Parse(e) => Some(e),
			_ => None,
		}
	}
}

impl From<kdl::KdlError> for Error {
	fn from(e: kdl::KdlError) -> Self {
		Error::Parse(e)
	}
}

/// Parse a KDL keybindings block into a Config.
///
/// Expects a document with action nodes where:
/// - Node name = action name (must match `T::from_str`)
/// - Arguments = key bindings (strings)
/// - `description` property = optional description
///
/// # Example
///
/// ```
/// use xeno_keymap::kdl::parse_keybindings;
/// use xeno_keymap::Config;
///
/// let kdl = r#"
///     Quit "q" "esc"
///     Save "ctrl-s" description="Save file"
/// "#;
///
/// let config: Config<String> = parse_keybindings(kdl).unwrap();
/// assert!(config.get_item_by_key_str("q").is_some());
/// ```
pub fn parse_keybindings<T>(input: &str) -> Result<Config<T>, Error>
where
	T: FromStr,
	T::Err: std::fmt::Display,
{
	let doc: KdlDocument = input.parse()?;
	parse_document(&doc)
}

/// Parse keybindings from a KdlDocument.
pub fn parse_document<T>(doc: &KdlDocument) -> Result<Config<T>, Error>
where
	T: FromStr,
	T::Err: std::fmt::Display,
{
	let mut items = Vec::new();

	for node in doc.nodes() {
		let (action, item) = parse_action_node(node)?;
		items.push((action, item));
	}

	Ok(Config::new(items))
}

/// Parse a single action node into (T, Item).
fn parse_action_node<T>(node: &KdlNode) -> Result<(T, Item), Error>
where
	T: FromStr,
	T::Err: std::fmt::Display,
{
	let action_name = node.name().value();

	let action: T = action_name
		.parse()
		.map_err(|e: T::Err| Error::UnknownAction(format!("{action_name}: {e}")))?;

	let mut keys = Vec::new();
	for entry in node.entries() {
		if entry.name().is_none()
			&& let Some(s) = entry.value().as_string()
		{
			xeno_keymap_parser::parse_seq(s).map_err(|e| Error::InvalidKey {
				action: action_name.to_string(),
				key: s.to_string(),
				reason: e.to_string(),
			})?;
			keys.push(s.to_string());
		}
	}

	let description = node
		.get("description")
		.and_then(|v| v.as_string())
		.unwrap_or("")
		.to_string();

	Ok((action, Item::new(keys, description)))
}

/// Parse keybindings from a named child block within a document.
///
/// Useful when keybindings are nested under a parent node:
///
/// ```kdl
/// config {
///     keybindings {
///         Quit "q"
///     }
/// }
/// ```
pub fn parse_keybindings_block<T>(doc: &KdlDocument, block_name: &str) -> Result<Config<T>, Error>
where
	T: FromStr,
	T::Err: std::fmt::Display,
{
	let node = doc
		.get(block_name)
		.ok_or_else(|| Error::MissingNode(block_name.to_string()))?;

	let children = node
		.children()
		.ok_or_else(|| Error::MissingNode(format!("{block_name} children")))?;

	parse_document(children)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_simple_bindings() {
		let kdl = r#"
            Quit "q" "esc"
            Save "ctrl-s"
        "#;

		let config: Config<String> = parse_keybindings(kdl).unwrap();

		let (action, item) = config.get_item_by_key_str("q").unwrap();
		assert_eq!(action, "Quit");
		assert!(item.keys.contains(&"q".to_string()));
		assert!(item.keys.contains(&"esc".to_string()));

		let (action, _) = config.get_item_by_key_str("ctrl-s").unwrap();
		assert_eq!(action, "Save");
	}

	#[test]
	fn parse_with_description() {
		let kdl = r#"
            Save "ctrl-s" description="Save the current file"
        "#;

		let config: Config<String> = parse_keybindings(kdl).unwrap();
		let (_, item) = config.get_item_by_key_str("ctrl-s").unwrap();
		assert_eq!(item.description, "Save the current file");
	}

	#[test]
	fn parse_key_sequences() {
		let kdl = r#"
            GotoLine "g l"
            GotoTop "g g"
        "#;

		let config: Config<String> = parse_keybindings(kdl).unwrap();

		let (action, _) = config.get_item_by_key_str("g l").unwrap();
		assert_eq!(action, "GotoLine");

		let (action, _) = config.get_item_by_key_str("g g").unwrap();
		assert_eq!(action, "GotoTop");
	}

	#[test]
	fn parse_key_groups() {
		let kdl = r#"
            GotoLine "@digit"
        "#;

		let config: Config<String> = parse_keybindings(kdl).unwrap();

		let (action, _) = config.get_item_by_key_str("5").unwrap();
		assert_eq!(action, "GotoLine");
	}

	#[test]
	fn parse_nested_block() {
		let kdl = r#"
            keybindings {
                Quit "q"
                Save "ctrl-s"
            }
        "#;

		let doc: KdlDocument = kdl.parse().unwrap();
		let config: Config<String> = parse_keybindings_block(&doc, "keybindings").unwrap();

		assert!(config.get_item_by_key_str("q").is_some());
		assert!(config.get_item_by_key_str("ctrl-s").is_some());
	}

	#[test]
	fn invalid_key_syntax() {
		let kdl = r#"
            Save "ctrl--s"
        "#;

		let result: Result<Config<String>, _> = parse_keybindings(kdl);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, Error::InvalidKey { .. }));
	}
}
