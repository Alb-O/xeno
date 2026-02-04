//! LSP routing and lifecycle management service.

mod commands;
mod handle;
mod lsp_doc;
mod service;
mod types;

pub use commands::RoutingCmd;
pub use handle::RoutingHandle;
pub use service::RoutingService;
