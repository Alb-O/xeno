#![cfg_attr(not(feature = "os"), allow(unused))]
#![doc = include_str!("../README.md")]
mod core_commands;
mod default_context;

pub use core_commands::*;
pub use default_context::*;

#[cfg(test)]
pub fn test_examples(_cmd: impl xeno_nu_protocol::engine::Command + 'static) {}
