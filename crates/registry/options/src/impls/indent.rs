//! Indentation-related options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "tab-width", scope = buffer)]
/// Number of spaces a tab character occupies for display.
pub static TAB_WIDTH: i64 = 4;

#[derive_option]
#[option(kdl = "indent-width", scope = buffer)]
/// Number of spaces per indentation level.
pub static INDENT_WIDTH: i64 = 4;

#[derive_option]
#[option(kdl = "use-tabs", scope = buffer)]
/// Use tabs instead of spaces for indentation.
pub static USE_TABS: bool = false;
