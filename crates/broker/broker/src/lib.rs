//! Xeno broker library for managing LSP servers and AI providers.

#![warn(missing_docs)]

pub mod core;
pub mod ipc;
pub mod lsp;
pub mod service;

pub use xeno_broker_proto as proto;
pub use xeno_broker_proto::protocol;
