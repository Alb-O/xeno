use serde::Serialize;

use crate::buffer::Buffer;

/// Test event emitted when viewport scrolling occurs.
#[derive(Serialize)]
pub(super) struct ViewportEnsureEvent {
	/// Event type identifier.
	#[serde(rename = "type")]
	pub kind: &'static str,
	/// Action taken (scroll_up, scroll_down, suppress_scroll_down, etc.).
	pub action: &'static str,
	/// ID of the buffer being scrolled.
	pub buffer_id: u64,
	/// Current viewport height in lines.
	pub viewport_height: usize,
	/// Previous viewport height before resize.
	pub prev_viewport_height: usize,
	/// Line number at top of viewport.
	pub scroll_line: usize,
	/// Wrap segment at top of viewport.
	pub scroll_segment: usize,
	/// Line number of the cursor.
	pub cursor_line: usize,
	/// Wrap segment of the cursor.
	pub cursor_segment: usize,
	/// Whether the viewport is shrinking.
	pub viewport_shrinking: bool,
	/// Whether auto-scrolling is suppressed.
	pub suppress_auto_scroll: bool,
}

impl ViewportEnsureEvent {
	/// Logs a viewport event for testing purposes.
	pub fn log(
		action: &'static str,
		buffer: &Buffer,
		viewport_height: usize,
		prev_viewport_height: usize,
		cursor_line: usize,
		cursor_segment: usize,
		viewport_shrinking: bool,
	) {
		crate::test_events::write_test_event(&Self {
			kind: "viewport_ensure",
			action,
			buffer_id: buffer.id.0,
			viewport_height,
			prev_viewport_height,
			scroll_line: buffer.scroll_line,
			scroll_segment: buffer.scroll_segment,
			cursor_line,
			cursor_segment,
			viewport_shrinking,
			suppress_auto_scroll: buffer.suppress_auto_scroll,
		});
	}
}
