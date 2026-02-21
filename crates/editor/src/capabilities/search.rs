use xeno_primitives::SeqDirection;
use xeno_registry::actions::SearchAccess;

use crate::capabilities::provider::EditorCaps;
use crate::overlay::LayerEvent;

impl SearchAccess for EditorCaps<'_> {
	fn search(&mut self, direction: SeqDirection, add_selection: bool, extend: bool) -> bool {
		let view = self.ed.focused_view();
		let found = match direction {
			SeqDirection::Next => self.ed.do_search_next(add_selection, extend),
			SeqDirection::Prev => self.ed.do_search_prev(add_selection, extend),
		};
		if found {
			self.ed.snippet_session_on_cursor_moved(view);
			self.ed.state.runtime.effects.push_layer_event(LayerEvent::CursorMoved { view });
		}
		found
	}

	fn search_repeat(&mut self, flip: bool, add_selection: bool, extend: bool) -> bool {
		let view = self.ed.focused_view();
		let found = self.ed.do_search_repeat(flip, add_selection, extend);
		if found {
			self.ed.snippet_session_on_cursor_moved(view);
			self.ed.state.runtime.effects.push_layer_event(LayerEvent::CursorMoved { view });
		}
		found
	}

	fn use_selection_as_pattern(&mut self) -> bool {
		let view = self.ed.focused_view();
		let found = self.ed.do_use_selection_as_search();
		if found {
			self.ed.snippet_session_on_cursor_moved(view);
			self.ed.state.runtime.effects.push_layer_event(LayerEvent::CursorMoved { view });
		}
		found
	}

	fn pattern(&self) -> Option<&str> {
		self.ed.buffer().input.last_search().map(|(p, _)| p)
	}

	fn set_pattern(&mut self, pattern: &str) {
		self.ed.buffer_mut().input.set_last_search(pattern.to_string(), false);
	}
}
