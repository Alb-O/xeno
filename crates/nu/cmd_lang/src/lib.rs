#![doc = include_str!("../README.md")]
mod core_commands;

pub use core_commands::*;

#[cfg(test)]
pub fn test_examples(_cmd: impl xeno_nu_protocol::engine::Command + 'static) {}
