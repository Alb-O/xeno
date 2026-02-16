//! Shared picker engine primitives used by modal picker overlays.
//!
//! This module centralizes reusable parsing, selection, and action models so
//! picker-like overlays can share one behavior contract.

pub mod apply;
pub mod decision;
pub mod model;
pub mod parser;
pub mod providers;

#[cfg(test)]
mod tests;
