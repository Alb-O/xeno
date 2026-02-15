//! Per-row render indices.
//!
//! Provides lookup structures for syntax-highlight spans and selection/cursor
//! overlays during row rendering.

pub mod highlight;
pub mod overlay;

pub use highlight::HighlightIndex;
pub use overlay::{CursorKind, OverlayIndex};
