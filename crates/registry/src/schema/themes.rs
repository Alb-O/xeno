//! Theme specification schema.
//!
//! Defines palette, UI/mode/semantic/popup colors, and syntax style mappings
//! for declarative theme definitions.


use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpec {
	/// Common metadata.
	pub common: MetaCommonSpec,
	/// Whether it's a "dark" or "light" theme.
	#[serde(default = "default_variant")]
	pub variant: String,
	/// Resolved color palette: Map of name -> hex string.
	#[serde(default)]
	pub palette: std::collections::HashMap<String, String>,
	/// UI colors: Map of field -> color name or hex.
	#[serde(default)]
	pub ui: std::collections::HashMap<String, String>,
	/// Mode colors: Map of field -> color name or hex.
	#[serde(default)]
	pub mode: std::collections::HashMap<String, String>,
	/// Semantic colors: Map of field -> color name or hex.
	#[serde(default)]
	pub semantic: std::collections::HashMap<String, String>,
	/// Popup colors: Map of field -> color name or hex.
	#[serde(default)]
	pub popup: std::collections::HashMap<String, String>,
	/// Syntax styles: Map of scope -> raw style.
	#[serde(default)]
	pub syntax: std::collections::HashMap<String, RawStyle>,
}

fn default_variant() -> String {
	"dark".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStyle {
	#[serde(default)]
	pub fg: Option<String>,
	#[serde(default)]
	pub bg: Option<String>,
	#[serde(default)]
	pub modifiers: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemesSpec {
	#[serde(default)]
	pub themes: Vec<ThemeSpec>,
}
