//! Buffer rendering for split views.
//!
//! This module provides buffer-agnostic rendering that can render any buffer
//! given a `BufferRenderContext`. This enables proper split view rendering
//! where multiple buffers are rendered simultaneously.

mod cell_style;
pub mod context;
mod diagnostics;
mod diff;
mod fill;
mod gutter;
mod index;
mod plan;
mod row;
mod style_layers;
mod viewport;

pub use context::{BufferRenderContext, RenderResult};
pub use diagnostics::{DiagnosticLineMap, DiagnosticRangeMap, DiagnosticSpan};
pub use viewport::ensure_buffer_cursor_visible;
