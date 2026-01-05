//! Display-related options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "line-numbers", scope = global)]
/// Show line numbers in the gutter.
pub static LINE_NUMBERS: bool = true;

#[derive_option]
#[option(kdl = "wrap-lines", scope = buffer)]
/// Wrap long lines at window edge.
pub static WRAP_LINES: bool = true;

#[derive_option]
#[option(kdl = "cursorline", scope = global)]
/// Highlight the current line.
pub static CURSORLINE: bool = true;

#[derive_option]
#[option(kdl = "cursorcolumn", scope = global)]
/// Highlight the current column.
pub static CURSORCOLUMN: bool = false;

#[derive_option]
#[option(kdl = "colorcolumn", scope = buffer)]
/// Column to highlight as margin guide.
pub static COLORCOLUMN: i64 = 0;

#[derive_option]
#[option(kdl = "whitespace-visible", scope = global)]
/// Show whitespace characters.
pub static WHITESPACE_VISIBLE: bool = false;
