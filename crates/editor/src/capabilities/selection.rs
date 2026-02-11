use xeno_primitives::Selection;
use xeno_registry::actions::SelectionAccess;

use crate::capabilities::provider::EditorCaps;
use crate::overlay::LayerEvent;

impl SelectionAccess for EditorCaps<'_> {
	fn selection(&self) -> &Selection {
		&self.ed.buffer().selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.ed.buffer_mut().selection
	}

	fn set_selection(&mut self, sel: Selection) {
		let view = self.ed.focused_view();
		self.ed.buffer_mut().set_selection(sel);
		self.ed.state.effects.push_layer_event(LayerEvent::CursorMoved { view });
	}
}
