//! Theme option.

use xeno_macro::derive_option;

/// Default theme ID (gruvbox).
pub const DEFAULT_THEME_ID: &str = "gruvbox";

#[derive_option]
#[option(kdl = "theme", scope = global)]
/// Editor color theme.
pub static THEME: &'static str = DEFAULT_THEME_ID;
