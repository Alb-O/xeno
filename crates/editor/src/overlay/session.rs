use std::collections::HashMap;

use xeno_primitives::range::{CharIdx, Range};
use xeno_primitives::{Mode, Selection};

use crate::buffer::ViewId;
use crate::impls::{Editor, FocusTarget};
use crate::window::WindowId;

pub struct OverlaySession {
	// Resources
	pub windows: Vec<WindowId>,
	pub buffers: Vec<ViewId>,
	pub input: ViewId,

	// Restoration
	pub origin_focus: FocusTarget,
	pub origin_mode: Mode,
	pub origin_view: ViewId,

	// Transient preview capture
	pub capture: PreviewCapture,

	pub status: OverlayStatus,
}

#[derive(Default)]
pub struct PreviewCapture {
	pub per_view: HashMap<ViewId, (CharIdx, Selection)>,
}

#[derive(Debug, Default, Clone)]
pub struct OverlayStatus {
	pub message: Option<(StatusKind, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
	Info,
	Warn,
	Error,
}

impl OverlaySession {
	pub fn input_text(&self, ed: &Editor) -> String {
		ed.state
			.core
			.buffers
			.get_buffer(self.input)
			.map(|b| b.with_doc(|doc| doc.content().to_string()))
			.unwrap_or_default()
	}

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

	pub fn preview_select(&mut self, ed: &mut Editor, view: ViewId, range: Range) {
		self.capture_view(ed, view);
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(view) {
			let start = range.min();
			let end = range.max();
			buffer.set_cursor(start);
			buffer.set_selection(Selection::single(start, end));
		}
	}

	pub fn restore_all(&mut self, ed: &mut Editor) {
		for (view, (cursor, selection)) in self.capture.per_view.drain() {
			if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(view) {
				buffer.set_cursor(cursor);
				buffer.set_selection(selection);
			}
		}
	}

	pub fn set_status(&mut self, kind: StatusKind, msg: impl Into<String>) {
		self.status.message = Some((kind, msg.into()));
	}

	pub fn clear_status(&mut self) {
		self.status.message = None;
	}
}
