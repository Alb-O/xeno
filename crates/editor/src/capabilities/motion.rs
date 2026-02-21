use xeno_primitives::Direction;
use xeno_registry::actions::MotionAccess;

use crate::capabilities::provider::EditorCaps;
use crate::overlay::LayerEvent;

impl MotionAccess for EditorCaps<'_> {
	fn move_visual_vertical(&mut self, direction: Direction, count: usize, extend: bool) {
		let view = self.ed.focused_view();
		self.ed.move_visual_vertical(direction, count, extend);
		self.ed.snippet_session_on_cursor_moved(view);
		self.ed.state.runtime.effects.push_layer_event(LayerEvent::CursorMoved { view });
	}
}
