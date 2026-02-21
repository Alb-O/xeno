//! Internal render-plan assembly.
//!
//! Builds data-only plans for document views, overlays, completion popups, and
//! wrapped text so frontends can render without policy logic.

mod buffer;
/// Render cache for efficient viewport rendering.
pub mod cache;
/// Completion popup rendering.
mod completion;
mod context;
#[cfg(test)]
mod document_plan;
mod snippet_choice;
mod text;
mod view_plan;
/// Line wrapping with sticky punctuation.
pub mod wrap;

#[cfg(any(feature = "lsp", test))]
pub use buffer::DiagnosticSpan;
pub use buffer::{BufferRenderContext, DiagnosticLineMap, DiagnosticRangeMap, GutterLayout, ensure_buffer_cursor_visible};
pub use text::{RenderLine, RenderSpan};
pub use view_plan::{DocumentViewPlan, SeparatorJunctionTarget, SeparatorRenderTarget, SeparatorState};
pub use wrap::wrap_line;
