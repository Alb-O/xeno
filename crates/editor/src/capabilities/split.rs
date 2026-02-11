use xeno_primitives::direction::Axis;
use xeno_registry::actions::SplitOps;
use xeno_registry::actions::editor_ctx::SplitError;

use crate::capabilities::provider::EditorCaps;
use crate::impls::Editor;
use crate::overlay::LayerEvent;

impl SplitOps for EditorCaps<'_> {
	fn split(&mut self, axis: Axis) -> Result<(), SplitError> {
		let res = match axis {
			Axis::Horizontal => Editor::split_horizontal_with_clone(self.ed),
			Axis::Vertical => Editor::split_vertical_with_clone(self.ed),
		};

		if res.is_ok() {
			self.ed.state.effects.push_layer_event(LayerEvent::LayoutChanged);
		}

		res.map_err(|e| match e {
			crate::layout::SplitError::ViewNotFound => SplitError::ViewNotFound,
			crate::layout::SplitError::AreaTooSmall => SplitError::AreaTooSmall,
		})
	}

	fn close_split(&mut self) {
		self.ed.close_current_buffer();
		self.ed.state.effects.push_layer_event(LayerEvent::LayoutChanged);
	}

	fn close_other_buffers(&mut self) {
		let current_id = self.ed.focused_view();
		let mut closed = false;
		for id in self.ed.buffer_ids() {
			if id != current_id {
				Editor::close_buffer(self.ed, id);
				closed = true;
			}
		}
		if closed {
			self.ed.state.effects.push_layer_event(LayerEvent::LayoutChanged);
		}
	}
}
