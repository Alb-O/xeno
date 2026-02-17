#![doc = include_str!("../README.md")]
#![allow(clippy::result_large_err, reason = "ShellError is intentionally rich and shared across Nu command APIs")]
mod core_commands;

pub use core_commands::*;

#[cfg(test)]
pub fn test_examples(_cmd: impl xeno_nu_protocol::engine::Command + 'static) {}
