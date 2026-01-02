//! Language configuration parsing from KDL.
//!
//! Parses `languages.kdl` to extract language definitions including file types,
//! comment tokens, shebangs, and other metadata.

use kdl::{KdlDocument, KdlNode};
use thiserror::Error;

use crate::language::LanguageData;

/// Errors from language configuration parsing.
#[derive(Debug, Error)]
pub enum LanguageConfigError {
	/// KDL syntax error.
	#[error("failed to parse KDL: {0}")]
	KdlParse(#[from] kdl::KdlError),
	/// Language node is missing the required `name` property.
	#[error("language node missing 'name' property")]
	MissingName,
}

/// Result type for language configuration operations.
pub type Result<T> = std::result::Result<T, LanguageConfigError>;

/// Loads language configurations from the embedded `languages.kdl`.
pub fn load_language_configs() -> Result<Vec<LanguageData>> {
	parse_language_configs(evildoer_runtime::language::languages_kdl())
}

/// Parses language configurations from a KDL string.
pub fn parse_language_configs(input: &str) -> Result<Vec<LanguageData>> {
	let doc: KdlDocument = input.parse()?;
	let mut languages = Vec::new();

	for node in doc.nodes() {
		if node.name().value() == "language"
			&& let Some(lang) = parse_language_node(node)?
		{
			languages.push(lang);
		}
	}

	Ok(languages)
}

/// Parses a single language node into LanguageData.
fn parse_language_node(node: &KdlNode) -> Result<Option<LanguageData>> {
	let name = node
		.get("name")
		.and_then(|v| v.as_string())
		.ok_or(LanguageConfigError::MissingName)?
		.to_string();

	let grammar = node
		.get("grammar")
		.and_then(|v| v.as_string())
		.map(String::from);
	let injection_regex = node.get("injection-regex").and_then(|v| v.as_string());

	let children = node.children();
	let (extensions, filenames, globs) = parse_file_types(children);
	let shebangs = parse_string_args(children, "shebangs");

	let mut comment_tokens = Vec::new();
	if let Some(token) = node.get("comment-token").and_then(|v| v.as_string()) {
		comment_tokens.push(token.to_string());
	}
	comment_tokens.extend(parse_string_args(children, "comment-tokens"));

	let block_comment = parse_block_comment(node, children);

	Ok(Some(LanguageData::new(
		name,
		grammar,
		extensions,
		filenames,
		globs,
		shebangs,
		comment_tokens,
		block_comment,
		injection_regex,
	)))
}

/// Parses file-types from either simple args (`file-types rs toml`) or
/// complex block with globs (`file-types { - rs; - glob=Cargo.lock }`).
fn parse_file_types(
	children: Option<&kdl::KdlDocument>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
	let mut extensions = Vec::new();
	let mut filenames = Vec::new();
	let mut globs = Vec::new();

	let Some(children) = children else {
		return (extensions, filenames, globs);
	};
	let Some(file_types_node) = children.get("file-types") else {
		return (extensions, filenames, globs);
	};

	for entry in file_types_node.entries() {
		if entry.name().is_none()
			&& let Some(s) = entry.value().as_string()
		{
			extensions.push(s.to_string());
		}
	}

	if let Some(ft_children) = file_types_node.children() {
		for child in ft_children.nodes() {
			if child.name().value() != "-" {
				continue;
			}
			if let Some(glob) = child.get("glob").and_then(|v| v.as_string()) {
				if glob.contains('*') || glob.contains('?') || glob.contains('[') {
					globs.push(glob.to_string());
				} else {
					filenames.push(glob.to_string());
				}
			} else if let Some(s) = child.entry(0).and_then(|e| e.value().as_string()) {
				extensions.push(s.to_string());
			}
		}
	}

	(extensions, filenames, globs)
}

/// Extracts string arguments from a named child node.
fn parse_string_args(children: Option<&kdl::KdlDocument>, name: &str) -> Vec<String> {
	children
		.and_then(|c| c.get(name))
		.map(|node| {
			node.entries()
				.iter()
				.filter(|e| e.name().is_none())
				.filter_map(|e| e.value().as_string())
				.map(String::from)
				.collect()
		})
		.unwrap_or_default()
}

/// Parses block comment tokens from node properties or children.
fn parse_block_comment(
	node: &KdlNode,
	children: Option<&kdl::KdlDocument>,
) -> Option<(String, String)> {
	if let (Some(start), Some(end)) = (
		node.get("block-comment-start").and_then(|v| v.as_string()),
		node.get("block-comment-end").and_then(|v| v.as_string()),
	) {
		return Some((start.to_string(), end.to_string()));
	}

	let bc_node = children?.get("block-comment-tokens")?;

	if let (Some(start), Some(end)) = (
		bc_node.get("start").and_then(|v| v.as_string()),
		bc_node.get("end").and_then(|v| v.as_string()),
	) {
		return Some((start.to_string(), end.to_string()));
	}

	bc_node.children().and_then(|bc_children| {
		bc_children.nodes().iter().find_map(|child| {
			if child.name().value() != "-" {
				return None;
			}
			let start = child.get("start").and_then(|v| v.as_string())?;
			let end = child.get("end").and_then(|v| v.as_string())?;
			Some((start.to_string(), end.to_string()))
		})
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_simple_language() {
		let kdl = r#"
language name=rust scope=source.rust injection-regex="rs|rust" {
    file-types rs
    shebangs rust-script cargo
    comment-tokens "//" "///"
    block-comment-tokens start="/*" end="*/"
}
"#;
		let langs = parse_language_configs(kdl).unwrap();
		assert_eq!(langs.len(), 1);

		let rust = &langs[0];
		assert_eq!(rust.name, "rust");
		assert_eq!(rust.extensions, vec!["rs"]);
		assert_eq!(rust.shebangs, vec!["rust-script", "cargo"]);
		assert_eq!(rust.comment_tokens, vec!["//", "///"]);
		assert_eq!(
			rust.block_comment,
			Some(("/*".to_string(), "*/".to_string()))
		);
		assert!(rust.injection_regex.is_some());
	}

	#[test]
	fn parse_complex_file_types() {
		let kdl = r#"
language name=toml scope=source.toml {
    file-types {
        - toml
        - glob=pdm.lock
        - glob=poetry.lock
        - glob="*.config"
    }
}
"#;
		let langs = parse_language_configs(kdl).unwrap();
		assert_eq!(langs.len(), 1);

		let toml = &langs[0];
		assert_eq!(toml.extensions, vec!["toml"]);
		assert_eq!(toml.filenames, vec!["pdm.lock", "poetry.lock"]);
		assert_eq!(toml.globs, vec!["*.config"]);
	}

	#[test]
	fn parse_comment_token_on_node() {
		let kdl = r##"
language name=python scope=source.python comment-token="#" {
    file-types py
}
"##;
		let langs = parse_language_configs(kdl).unwrap();
		assert_eq!(langs[0].comment_tokens, vec!["#"]);
	}

	#[test]
	fn load_embedded_languages() {
		let langs = load_language_configs().expect("embedded languages.kdl should parse");
		assert!(!langs.is_empty());

		let rust = langs
			.iter()
			.find(|l| l.name == "rust")
			.expect("rust language");
		assert!(rust.extensions.contains(&"rs".to_string()));
	}
}
