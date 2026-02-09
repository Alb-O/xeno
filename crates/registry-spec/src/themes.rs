use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpec {
	/// Common metadata.
	pub common: MetaCommonSpec,
	/// Whether it's a "dark" or "light" theme.
	pub variant: String,
	/// Resolved color palette: Map of name -> hex string.
	pub palette: std::collections::HashMap<String, String>,
	/// UI colors: Map of field -> color name or hex.
	pub ui: std::collections::HashMap<String, String>,
	/// Mode colors: Map of field -> color name or hex.
	pub mode: std::collections::HashMap<String, String>,
	/// Semantic colors: Map of field -> color name or hex.
	pub semantic: std::collections::HashMap<String, String>,
	/// Popup colors: Map of field -> color name or hex.
	pub popup: std::collections::HashMap<String, String>,
	/// Syntax styles: Map of scope -> raw style.
	pub syntax: std::collections::HashMap<String, RawStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStyle {
	pub fg: Option<String>,
	pub bg: Option<String>,
	pub modifiers: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemesSpec {
	pub themes: Vec<ThemeSpec>,
}
