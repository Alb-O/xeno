mod buffer;
/// Render cache for efficient viewport rendering.
pub mod cache;
/// Completion popup rendering.
mod completion;
mod context;
mod snippet_choice;
mod text;
/// Line wrapping with sticky punctuation.
pub mod wrap;

#[cfg(any(feature = "lsp", test))]
pub use buffer::DiagnosticSpan;
pub use buffer::{BufferRenderContext, DiagnosticLineMap, DiagnosticRangeMap, GutterLayout, RenderBufferParams, ensure_buffer_cursor_visible};
pub use context::RenderCtx;
pub use text::{RenderLine, RenderSpan};
pub use wrap::wrap_line;
