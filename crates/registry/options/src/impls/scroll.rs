//! Scrolling behavior options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "scroll-margin", scope = global)]
/// Minimum lines to keep above/below cursor when scrolling.
pub static SCROLL_MARGIN: i64 = 3;

#[derive_option]
#[option(kdl = "scroll-smooth", scope = global)]
/// Enable smooth scrolling animations.
pub static SCROLL_SMOOTH: bool = false;
