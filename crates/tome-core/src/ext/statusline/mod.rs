//! Statusline segment system for extensible status bar rendering.
//!
//! Segments are registered at compile-time using `linkme` and rendered
//! in order based on their position and priority.

mod count;
mod file;
mod filetype;
mod mode;
mod position;
mod progress;

use linkme::distributed_slice;

/// Registry of all statusline segment definitions.
#[distributed_slice]
pub static STATUSLINE_SEGMENTS: [StatuslineSegmentDef];

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
	/// Unique name for the segment.
	pub name: &'static str,
	/// Position in the statusline.
	pub position: SegmentPosition,
	/// Priority within position (lower = renders first).
	pub priority: i16,
	/// Whether this segment is enabled by default.
	pub default_enabled: bool,
	/// Render function.
	pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
}

impl std::fmt::Debug for StatuslineSegmentDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("StatuslineSegmentDef")
			.field("name", &self.name)
			.field("position", &self.position)
			.field("priority", &self.priority)
			.finish()
	}
}

/// Get all segments for a given position, sorted by priority.
pub fn segments_for_position(
	position: SegmentPosition,
) -> impl Iterator<Item = &'static StatuslineSegmentDef> {
	let mut segments: Vec<_> = STATUSLINE_SEGMENTS
		.iter()
		.filter(move |s| s.position == position && s.default_enabled)
		.collect();
	segments.sort_by_key(|s| s.priority);
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
	STATUSLINE_SEGMENTS.iter().find(|s| s.name == name)
}

/// Get all registered segments.
pub fn all_segments() -> &'static [StatuslineSegmentDef] {
	&STATUSLINE_SEGMENTS
}

/// Helper macro to define statusline segments consistently.
#[macro_export]
macro_rules! statusline_segment {
	($static_name:ident, $name:literal, $position:expr, $priority:expr, $default_enabled:expr, $render:expr) => {
		#[::linkme::distributed_slice($crate::ext::statusline::STATUSLINE_SEGMENTS)]
		static $static_name: $crate::ext::statusline::StatuslineSegmentDef =
			$crate::ext::statusline::StatuslineSegmentDef {
				name: $name,
				position: $position,
				priority: $priority,
				default_enabled: $default_enabled,
				render: $render,
			};
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_segments_registered() {
		let segments = all_segments();
		assert!(
			segments.len() >= 4,
			"should have at least 4 default segments"
		);
	}

	#[test]
	fn test_find_segment() {
		assert!(find_segment("mode").is_some());
		assert!(find_segment("file").is_some());
		assert!(find_segment("position").is_some());
	}

	#[test]
	fn test_render_position() {
		let ctx = StatuslineContext {
			mode_name: "NORMAL",
			path: Some("test.rs"),
			modified: false,
			line: 1,
			col: 1,
			count: 0,
			total_lines: 100,
			file_type: Some("rust"),
		};

		let left = render_position(SegmentPosition::Left, &ctx);
		assert!(!left.is_empty(), "should have left segments");
	}
}
