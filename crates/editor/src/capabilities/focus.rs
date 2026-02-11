use xeno_primitives::direction::{SeqDirection, SpatialDirection};
use xeno_registry::actions::FocusOps;

use crate::capabilities::provider::EditorCaps;
use crate::overlay::LayerEvent;

impl FocusOps for EditorCaps<'_> {
	fn buffer_switch(&mut self, direction: SeqDirection) {
		match direction {
			SeqDirection::Next => self.ed.focus_next_buffer(),
			SeqDirection::Prev => self.ed.focus_prev_buffer(),
		}
		let view = self.ed.focused_view();
		self.ed.snippet_session_on_cursor_moved(view);
		self.ed.state.effects.push_layer_event(LayerEvent::CursorMoved { view });
	}

	fn focus(&mut self, direction: SpatialDirection) {
		self.ed.focus_direction(direction);
		let view = self.ed.focused_view();
		self.ed.snippet_session_on_cursor_moved(view);
		self.ed.state.effects.push_layer_event(LayerEvent::CursorMoved { view });
	}
}
