//! Theme option.

use crate::option;

/// Default theme ID (gruvbox).
pub const DEFAULT_THEME_ID: &str = "gruvbox";

option!(
	theme,
	String,
	DEFAULT_THEME_ID.to_string(),
	Global,
	"Editor color theme"
);
