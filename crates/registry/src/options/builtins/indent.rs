//! Indentation-related options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "tab-width", scope = buffer, validate = positive_int)]
/// Number of spaces a tab character occupies for display.
pub static TAB_WIDTH: i64 = 4;
