mod buffer;
/// Completion popup rendering.
mod completion;
mod context;
mod document;
/// Status line rendering.
mod status;
/// Line wrapping with sticky punctuation.
pub mod wrap;

pub use buffer::{
	BufferRenderContext, DiagnosticLineMap, DiagnosticRangeMap, DiagnosticSpan, RenderResult,
	ensure_buffer_cursor_visible,
};
pub use context::{LayoutSnapshot, LspRenderSnapshot, RenderCtx};
pub use wrap::{WrapSegment, wrap_line};
