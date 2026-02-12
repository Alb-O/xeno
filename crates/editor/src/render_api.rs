//! Frontend-facing render API boundary.
//!
//! This module re-exports the minimal render types/functions consumed by
//! frontend crates so coupling stays explicit and reviewable.

pub use crate::render::{BufferRenderContext, GutterLayout, RenderBufferParams, RenderCtx, RenderLine, RenderSpan, ensure_buffer_cursor_visible};
