use xeno_tui::Frame;
use xeno_tui::layout::Rect;

use crate::impls::Editor;
use crate::info_popup::InfoPopupStore;
use crate::overlay::{LayerEvent, OverlayLayer};

pub struct InfoPopupLayer;

impl OverlayLayer for InfoPopupLayer {
	fn name(&self) -> &'static str {
		"InfoPopup"
	}

	fn is_visible(&self, ed: &Editor) -> bool {
		ed.overlays()
			.get::<InfoPopupStore>()
			.is_some_and(|s| !s.is_empty())
	}

	fn layout(&self, _ed: &Editor, _screen: Rect) -> Option<Rect> {
		// InfoPopups manage their own rects in FloatingWindows for now.
		// We return None here because we are not using the layer's render path yet.
		None
	}

	fn render(&self, _ed: &Editor, _frame: &mut Frame, _area: Rect) {
		// Handled by window manager
	}

	fn on_event(&mut self, ed: &mut Editor, event: &LayerEvent) {
		match event {
			LayerEvent::CursorMoved { .. }
			| LayerEvent::ModeChanged { .. }
			| LayerEvent::FocusChanged { .. } => {
				ed.close_all_info_popups();
			}
			_ => {}
		}
	}
}
