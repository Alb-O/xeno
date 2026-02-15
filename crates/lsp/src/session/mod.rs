//! LSP session orchestration surface.
//!
//! Groups the session manager, completion controller wiring, and handlers for
//! server-initiated requests.

mod completion;
pub(crate) mod manager;
pub mod server_requests;

pub use completion::{CompletionController, CompletionRequest, CompletionTrigger};
pub use manager::LspManager;
