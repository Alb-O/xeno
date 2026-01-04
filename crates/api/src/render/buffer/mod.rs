//! Buffer rendering for split views.
//!
//! This module provides buffer-agnostic rendering that can render any buffer
//! given a `BufferRenderContext`. This enables proper split view rendering
//! where multiple buffers are rendered simultaneously.

mod context;
mod gutter;
mod viewport;

pub use context::BufferRenderContext;
pub use viewport::ensure_buffer_cursor_visible;
