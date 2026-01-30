//! Xeno broker library for managing LSP servers and AI providers.

#![warn(missing_docs)]

pub mod ipc;
pub mod protocol;
pub mod service;

pub use xeno_broker_proto as proto;
