//! Frontend-facing render API boundary.
//!
//! This module re-exports the minimal render types/functions consumed by
//! frontend crates so coupling stays explicit and reviewable.

pub use crate::render::{
	BufferViewRenderPlan, DocumentRenderPlan, GutterLayout, OverlayCompletionMenuTarget, RenderLine, RenderSpan, ensure_buffer_cursor_visible,
};
