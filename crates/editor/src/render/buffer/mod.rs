//! Buffer rendering for split views.
//!
//! This module provides buffer-agnostic rendering that can render any buffer
//! given a `BufferRenderContext`. This enables proper split view rendering
//! where multiple buffers are rendered simultaneously.

mod cell_style;
mod context;
mod diagnostics;
mod diff;
mod fill;
mod gutter;
mod style_layers;
mod viewport;

pub use context::{BufferRenderContext, RenderResult};
pub use diagnostics::DiagnosticLineMap;
#[cfg(feature = "lsp")]
#[allow(unused_imports, reason = "re-exported for public API completeness")]
pub use diagnostics::{DiagnosticRangeMap, build_diagnostic_line_map, build_diagnostic_range_map};
pub use viewport::ensure_buffer_cursor_visible;
