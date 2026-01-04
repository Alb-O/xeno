//! Theme option.

use crate::option;

/// Default theme ID (gruvbox).
pub const DEFAULT_THEME_ID: &str = "gruvbox";

option!(theme, {
	kdl: "theme",
	type: String,
	default: DEFAULT_THEME_ID.to_string(),
	scope: Global,
	description: "Editor color theme",
});
