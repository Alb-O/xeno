//! Result handlers for [`ActionResult`](xeno_core::ActionResult) variants.
//! Core ActionResult handlers for the editor.
//!
//! Extensions should register handlers with `result_extension_handler!`.

mod core;
mod edit;
mod mode;
mod search;
