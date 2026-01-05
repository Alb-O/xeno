//! Editor behavior options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "mouse", scope = global)]
/// Enable mouse support.
pub static MOUSE: bool = true;

#[derive_option]
#[option(kdl = "line-ending", scope = buffer)]
/// Default line ending (lf, crlf, cr).
pub static LINE_ENDING: &'static str = "lf";

#[derive_option]
#[option(kdl = "idle-timeout", scope = global)]
/// Milliseconds before triggering idle hooks.
pub static IDLE_TIMEOUT: i64 = 250;
