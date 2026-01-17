//! Statusline segment registry.
//!
//! Segments are rendered in order based on their position and priority.

use std::collections::HashMap;
use std::sync::LazyLock;

mod impls;
mod macros;

pub use xeno_registry_core::{
	RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource, impl_registry_entry,
};

/// Wrapper for [`inventory`] collection of statusline segment definitions.
pub struct StatuslineSegmentReg(pub &'static StatuslineSegmentDef);
inventory::collect!(StatuslineSegmentReg);

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

/// O(1) segment lookup index by name.
static SEGMENT_INDEX: LazyLock<HashMap<&'static str, &'static StatuslineSegmentDef>> =
	LazyLock::new(|| {
		let mut map = HashMap::new();
		for reg in inventory::iter::<StatuslineSegmentReg> {
			map.insert(reg.0.meta.name, reg.0);
		}
		map
	});

/// Lazy reference to all segments for iteration.
pub static STATUSLINE_SEGMENTS: LazyLock<Vec<&'static StatuslineSegmentDef>> = LazyLock::new(|| {
	let mut segs: Vec<_> = inventory::iter::<StatuslineSegmentReg>().map(|r| r.0).collect();
	segs.sort_by_key(|s| s.meta.priority);
	segs
});

/// Get all segments for a given position, sorted by priority.
pub fn segments_for_position(
	position: SegmentPosition,
) -> impl Iterator<Item = &'static StatuslineSegmentDef> {
	STATUSLINE_SEGMENTS
		.iter()
		.copied()
		.filter(move |s| s.position == position && s.default_enabled)
}

/// Render all segments for a position.
pub fn render_position(position: SegmentPosition, ctx: &StatuslineContext) -> Vec<RenderedSegment> {
	segments_for_position(position)
		.filter_map(|seg| (seg.render)(ctx))
		.collect()
}

/// Find a segment by name.
pub fn find_segment(name: &str) -> Option<&'static StatuslineSegmentDef> {
	SEGMENT_INDEX.get(name).copied()
}

/// Get all registered segments, sorted by priority.
pub fn all_segments() -> impl Iterator<Item = &'static StatuslineSegmentDef> {
	STATUSLINE_SEGMENTS.iter().copied()
}

impl_registry_entry!(StatuslineSegmentDef);
