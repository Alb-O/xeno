use xeno_primitives::range::CharIdx;
use xeno_registry::actions::CursorAccess;

use crate::capabilities::provider::EditorCaps;
use crate::overlay::LayerEvent;

impl CursorAccess for EditorCaps<'_> {
	fn focused_view(&self) -> xeno_registry::hooks::ViewId {
		self.ed.focused_view()
	}

	fn cursor(&self) -> CharIdx {
		self.ed.buffer().cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		let buffer = self.ed.buffer();
		Some((buffer.cursor_line(), buffer.cursor_col()))
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		let view = self.ed.focused_view();
		self.ed.buffer_mut().set_cursor(pos);
		self.ed.snippet_session_on_cursor_moved(view);
		self.ed.state.effects.push_layer_event(LayerEvent::CursorMoved { view });
	}
}
