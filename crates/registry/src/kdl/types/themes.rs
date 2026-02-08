use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Raw theme metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeMetaRaw {
	/// Theme name.
	pub name: String,
	/// Alternative lookup names.
	pub keys: Vec<String>,
	/// Human-readable description.
	pub description: String,
	/// Conflict resolution priority.
	pub priority: i16,
	/// Whether it's a "dark" or "light" theme.
	pub variant: String,
	/// Resolved color palette: Map of name -> hex string.
	pub palette: HashMap<String, String>,
	/// UI colors: Map of field -> color name or hex.
	pub ui: HashMap<String, String>,
	/// Mode colors: Map of field -> color name or hex.
	pub mode: HashMap<String, String>,
	/// Semantic colors: Map of field -> color name or hex.
	pub semantic: HashMap<String, String>,
	/// Popup colors: Map of field -> color name or hex.
	pub popup: HashMap<String, String>,
	/// Syntax styles: Map of scope -> raw style.
	pub syntax: HashMap<String, RawStyle>,
}

/// Serializable style definition.
#[derive(Debug, Serialize, Deserialize)]
pub struct RawStyle {
	pub fg: Option<String>,
	pub bg: Option<String>,
	pub modifiers: Option<String>,
}

/// Top-level blob containing all theme metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct ThemesBlob {
	/// All theme definitions.
	pub themes: Vec<ThemeMetaRaw>,
}
