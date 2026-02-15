#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Iced frontend facade for Xeno.
//!
//! Exposes runtime startup entry points when the `iced-wgpu` feature is
//! enabled. This crate intentionally keeps a thin public surface so frontend
//! integration details stay inside the runtime module tree.

#[cfg(feature = "iced-wgpu")]
mod runtime;

#[cfg(feature = "iced-wgpu")]
pub use runtime::{StartupOptions, run};
