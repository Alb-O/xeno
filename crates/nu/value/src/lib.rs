//! Minimal Nu value surface for non-vendor crates.
//!
//! This crate intentionally re-exports only the data-model pieces that Xeno
//! uses at integration boundaries.

pub use xeno_nu_protocol::{CustomValue, Record, ShellError, Span, Value};
