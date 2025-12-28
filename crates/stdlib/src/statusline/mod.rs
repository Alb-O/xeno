//! Statusline segment implementations.
//!
//! This module contains the built-in statusline segment implementations.
//! Type definitions are in evildoer-manifest.

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

/// Helper macro to define statusline segments consistently.
#[macro_export]
macro_rules! statusline_segment {
	($static_name:ident, $name:expr, $position:expr, $priority:expr, $enabled:expr, $render:expr) => {
		#[::linkme::distributed_slice(evildoer_manifest::STATUSLINE_SEGMENTS)]
		static $static_name: evildoer_manifest::StatuslineSegmentDef =
			evildoer_manifest::StatuslineSegmentDef {
				id: $name,
				name: $name,
				position: $position,
				priority: $priority,
				default_enabled: $enabled,
				render: $render,
				source: evildoer_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
	};
}
