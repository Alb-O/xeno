#![cfg_attr(test, allow(unused_crate_dependencies))]

#[cfg(feature = "iced-wgpu")]
mod runtime;

#[cfg(feature = "iced-wgpu")]
pub use runtime::{StartupOptions, run};
