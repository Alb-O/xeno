//! Scroll-related options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "scroll-lines", scope = global, validate = positive_int)]
/// Number of lines to scroll per mouse wheel tick.
pub static SCROLL_LINES: i64 = 2;
