//! Buffer rendering context and cursor styling.

mod ops;
#[cfg(all(test, feature = "tui"))]
mod tests;
pub mod types;

pub use types::{BufferRenderContext, RenderBufferParams, RenderResult};
