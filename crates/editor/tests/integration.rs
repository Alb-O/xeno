#![allow(unused_crate_dependencies)]

#[cfg(feature = "lsp")]
#[path = "integration/common/mod.rs"]
mod common;

#[cfg(feature = "lsp")]
#[path = "integration/broker_e2e.rs"]
mod broker_e2e;

#[cfg(feature = "lsp")]
#[path = "integration/broker_edge_cases.rs"]
mod broker_edge_cases;
