//! Buffer rendering for split views.
//!
//! This module provides buffer-agnostic rendering that can render any buffer
//! given a `BufferRenderContext`. This enables proper split view rendering
//! where multiple buffers are rendered simultaneously.

mod cell_style;
pub mod context;
mod diagnostics;
pub(crate) mod diff;
mod fill;
mod gutter;
mod index;
pub mod plan;
mod row;
mod style_layers;
mod viewport;

pub use context::{BufferRenderContext, RenderBufferParams};
#[cfg(any(feature = "lsp", test))]
pub use diagnostics::DiagnosticSpan;
pub use diagnostics::{DiagnosticLineMap, DiagnosticRangeMap};
pub use gutter::GutterLayout;
pub use viewport::ensure_buffer_cursor_visible;
