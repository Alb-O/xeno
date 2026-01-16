//! Statusline segment registry
//!
//! Segments are registered at compile-time using [`linkme`] and rendered
//! in order based on their position and priority.

use linkme::distributed_slice;

mod impls;
/// Statusline segment registration macros.
mod macros;

pub use xeno_registry_core::{
	RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource, impl_registry_entry,
};

/// Position in the statusline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentPosition {
	/// Left-aligned segments (mode, count, etc.)
	Left,
	/// Center segments (filename, etc.)
	Center,
	/// Right-aligned segments (position, diagnostics, etc.)
	Right,
}

/// Context provided to statusline segment renderers.
pub struct StatuslineContext<'a> {
	/// Current mode name.
	pub mode_name: &'a str,
	/// File path being edited.
	pub path: Option<&'a str>,
	/// Whether the buffer is modified.
	pub modified: bool,
	/// Whether the buffer is read-only.
	pub readonly: bool,
	/// Current line number (1-indexed).
	pub line: usize,
	/// Current column number (1-indexed).
	pub col: usize,
	/// Numeric count prefix (0 if not specified).
	pub count: u32,
	/// Total lines in document.
	pub total_lines: usize,
	/// File type name if detected.
	pub file_type: Option<&'a str>,
	/// Current buffer index (1-indexed).
	pub buffer_index: usize,
	/// Total number of open buffers.
	pub buffer_count: usize,
}

/// A rendered segment with styling information.
#[derive(Debug, Clone)]
pub struct RenderedSegment {
	/// The text content.
	pub text: String,
	/// Style hint for the segment.
	pub style: SegmentStyle,
}

/// Style hints for segments (actual colors handled by terminal layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SegmentStyle {
	/// Default style.
	#[default]
	Normal,
	/// Mode indicator style (varies by mode).
	Mode,
	/// Inverted/reversed style.
	Inverted,
	/// Dimmed/secondary style.
	Dim,
	/// Warning style.
	Warning,
	/// Error style.
	Error,
	/// Success/good style.
	Success,
}

/// Definition of a statusline segment.
pub struct StatuslineSegmentDef {
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Position in the statusline.
	pub position: SegmentPosition,
	/// Whether this segment is enabled by default.
	pub default_enabled: bool,
	/// Render function.
	pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
}

impl core::fmt::Debug for StatuslineSegmentDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("StatuslineSegmentDef")
			.field("name", &self.meta.name)
			.field("position", &self.position)
			.field("priority", &self.meta.priority)
			.finish()
	}
}

/// Registry of all statusline segment definitions.
#[distributed_slice]
pub static STATUSLINE_SEGMENTS: [StatuslineSegmentDef];

/// Get all segments for a given position, sorted by priority.
pub fn segments_for_position(
	position: SegmentPosition,
) -> impl Iterator<Item = &'static StatuslineSegmentDef> {
	let mut segments: Vec<_> = STATUSLINE_SEGMENTS
		.iter()
		.filter(move |s| s.position == position && s.default_enabled)
		.collect();
	segments.sort_by_key(|s| s.meta.priority);
	segments.into_iter()
}

/// Render all segments for a position.
pub fn render_position(position: SegmentPosition, ctx: &StatuslineContext) -> Vec<RenderedSegment> {
	segments_for_position(position)
		.filter_map(|seg| (seg.render)(ctx))
		.collect()
}

/// Find a segment by name.
pub fn find_segment(name: &str) -> Option<&'static StatuslineSegmentDef> {
	STATUSLINE_SEGMENTS.iter().find(|s| s.meta.name == name)
}

/// Get all registered segments.
pub fn all_segments() -> impl Iterator<Item = &'static StatuslineSegmentDef> {
	STATUSLINE_SEGMENTS.iter()
}

impl_registry_entry!(StatuslineSegmentDef);
