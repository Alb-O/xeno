//! Scroll-related options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "scroll-lines", scope = global, validate = positive_int)]
/// Number of lines to scroll per mouse wheel tick.
pub static SCROLL_LINES: i64 = 2;

#[derive_option]
#[option(kdl = "scroll-margin", scope = buffer, validate = positive_int)]
/// Minimum lines to keep above/below cursor when scrolling.
///
/// When the cursor moves within this many lines of the viewport edge,
/// the view scrolls to maintain the margin. At buffer boundaries, the
/// cursor is allowed to reach the edge.
pub static SCROLL_MARGIN: i64 = 5;
