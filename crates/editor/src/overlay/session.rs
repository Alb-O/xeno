use std::collections::HashMap;

use xeno_primitives::range::{CharIdx, Range};
use xeno_primitives::{Mode, Selection};

use crate::buffer::ViewId;
use crate::impls::{Editor, FocusTarget};
use crate::window::WindowId;

/// State and resources for an active modal interaction session.
///
/// An `OverlaySession` is created by [`OverlayHost`] and managed by [`OverlayManager`].
/// It tracks all allocated UI resources and provides mechanisms for temporary
/// state capture and restoration.
pub struct OverlaySession {
	/// List of floating window IDs allocated for this session.
	pub windows: Vec<WindowId>,
	/// List of scratch buffer IDs allocated for this session.
	pub buffers: Vec<ViewId>,
	/// The primary input buffer ID for the interaction.
	pub input: ViewId,

	/// The focus target to restore after the session ends.
	pub origin_focus: FocusTarget,
	/// The editor mode to restore after the session ends.
	pub origin_mode: Mode,
	/// The buffer view that was active when the session started.
	pub origin_view: ViewId,

	/// Storage for captured buffer states (cursor, selection) for restoration.
	pub capture: PreviewCapture,

	/// Current status message displayed by the overlay.
	pub status: OverlayStatus,
}

/// Storage for buffer states captured before transient changes.
#[derive(Default)]
pub struct PreviewCapture {
	/// Mapping of view ID to (cursor position, selection).
	pub per_view: HashMap<ViewId, (CharIdx, Selection)>,
}

/// Metadata about the current session status.
#[derive(Debug, Default, Clone)]
pub struct OverlayStatus {
	/// Optional status message and its severity kind.
	pub message: Option<(StatusKind, String)>,
}

/// Severity kind for overlay status messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
	Info,
	Warn,
	Error,
}

impl OverlaySession {
	/// Returns the current text content of the primary input buffer.
	pub fn input_text(&self, ed: &Editor) -> String {
		ed.state
			.core
			.buffers
			.get_buffer(self.input)
			.map(|b| b.with_doc(|doc| doc.content().to_string()))
			.unwrap_or_default()
	}

	/// Captures the current state of a view if it hasn't been captured yet.
	///
	/// Use this before applying preview modifications to a buffer to ensure
	/// the original state can be restored.
	pub fn capture_view(&mut self, ed: &Editor, view: ViewId) {
		if self.capture.per_view.contains_key(&view) {
			return;
		}
		if let Some(buffer) = ed.state.core.buffers.get_buffer(view) {
			self.capture
				.per_view
				.insert(view, (buffer.cursor, buffer.selection.clone()));
		}
	}

	/// Selects a range in a view, capturing its state first if necessary.
	pub fn preview_select(&mut self, ed: &mut Editor, view: ViewId, range: Range) {
		self.capture_view(ed, view);
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(view) {
			let start = range.min();
			let end = range.max();
			buffer.set_cursor(start);
			buffer.set_selection(Selection::single(start, end));
		}
	}

	/// Restores all captured view states.
	///
	/// This is non-destructive; the capture map remains intact until
	/// [`Self::clear_capture`] is called.
	pub fn restore_all(&self, ed: &mut Editor) {
		for (view, (cursor, selection)) in &self.capture.per_view {
			if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(*view) {
				buffer.set_cursor(*cursor);
				buffer.set_selection(selection.clone());
			}
		}
	}

	/// Destroys all captured view state.
	pub fn clear_capture(&mut self) {
		self.capture.per_view.clear();
	}

	/// Sets the session status message.
	pub fn set_status(&mut self, kind: StatusKind, msg: impl Into<String>) {
		self.status.message = Some((kind, msg.into()));
	}

	/// Clears the session status message.
	pub fn clear_status(&mut self) {
		self.status.message = None;
	}
}
