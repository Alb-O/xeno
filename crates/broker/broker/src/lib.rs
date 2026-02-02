#![warn(missing_docs)]

//! Xeno broker library for managing LSP servers and AI providers.

pub mod core;
pub mod ipc;
pub mod launcher;
pub mod lsp;
/// Broker runtime orchestration.
pub mod runtime;
pub mod service;
/// Background actor services.
pub mod services;
pub mod wire_convert;

#[doc(hidden)]
pub use launcher::test_helpers;
pub use xeno_broker_proto as proto;
pub use xeno_broker_proto::protocol;
