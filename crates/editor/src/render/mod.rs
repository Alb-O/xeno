mod buffer;
/// Render cache for efficient viewport rendering.
pub mod cache;
/// Completion popup rendering.
mod completion;
mod context;
mod document_plan;
mod snippet_choice;
mod text;
mod view_plan;
/// Line wrapping with sticky punctuation.
pub mod wrap;

#[cfg(any(feature = "lsp", test))]
pub use buffer::DiagnosticSpan;
pub use buffer::{BufferRenderContext, DiagnosticLineMap, DiagnosticRangeMap, GutterLayout, ensure_buffer_cursor_visible};
pub use completion::OverlayCompletionMenuTarget;
pub use document_plan::DocumentRenderPlan;
pub use text::{RenderLine, RenderSpan};
pub use view_plan::BufferViewRenderPlan;
pub use wrap::wrap_line;
