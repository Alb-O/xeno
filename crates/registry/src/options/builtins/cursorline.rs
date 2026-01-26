//! Cursorline display option.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "cursorline", scope = buffer)]
/// Whether to highlight the line containing the cursor.
pub static CURSORLINE: bool = true;
