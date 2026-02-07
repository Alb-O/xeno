use xeno_primitives::range::CharIdx;
use xeno_registry::ViewportAccess;

use crate::capabilities::provider::EditorCaps;

impl ViewportAccess for EditorCaps<'_> {
	fn viewport_height(&self) -> usize {
		self.ed.buffer().last_viewport_height
	}

	fn viewport_row_to_doc_position(&self, row: usize) -> Option<CharIdx> {
		let buffer = self.ed.buffer();
		if buffer.last_viewport_height == 0 {
			return None;
		}
		let tab_width = self.ed.tab_width();
		buffer
			.screen_to_doc_position(row as u16, buffer.gutter_width(), tab_width)
			.map(|pos| pos as CharIdx)
	}
}
