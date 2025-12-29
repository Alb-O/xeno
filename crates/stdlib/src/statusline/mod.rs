//! Statusline segment implementations.
//!
//! This module contains the built-in statusline segment implementations.
//! Type definitions and the [`statusline_segment!`](evildoer_manifest::statusline_segment)
//! macro are in evildoer-manifest.

mod count;
mod file;
mod filetype;
mod mode;
mod position;
mod progress;

// Re-export types from evildoer-manifest for use in segment implementations
pub use evildoer_manifest::statusline::{
	RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, all_segments, find_segment, render_position, segments_for_position,
};
