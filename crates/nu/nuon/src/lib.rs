#![doc = include_str!("../README.md")]
#![allow(clippy::result_large_err, reason = "ShellError is intentionally rich and shared across NUON conversion APIs")]
mod from;

pub use from::from_nuon;
