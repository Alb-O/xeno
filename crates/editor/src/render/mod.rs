mod buffer;
/// Render cache for efficient viewport rendering.
pub mod cache;
/// Completion popup rendering.
mod completion;
mod context;
mod document;
/// Status line rendering.
mod status;
/// Line wrapping with sticky punctuation.
pub mod wrap;

pub use buffer::{
	BufferRenderContext, DiagnosticLineMap, DiagnosticRangeMap, DiagnosticSpan, GutterLayout,
	LineSlice, LineSource, RenderResult, RowKind, ViewportPlan, WrapAccess,
	ensure_buffer_cursor_visible,
};
pub use cache::{
	DiagnosticsCache, DiagnosticsCacheKey, DiagnosticsEntry, HighlightKey, HighlightTile,
	HighlightTiles, RenderCache, TILE_SIZE, WrapBucket, WrapBucketKey, WrapBuckets, WrapEntry,
};
pub use context::{LayoutSnapshot, LspRenderSnapshot, RenderCtx};
pub use wrap::{WrapSegment, WrappedSegment, wrap_line};
