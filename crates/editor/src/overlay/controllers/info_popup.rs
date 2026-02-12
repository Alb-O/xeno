use xeno_tui::Frame;

use crate::geometry::Rect;
use crate::impls::Editor;
use crate::info_popup::InfoPopupStore;
use crate::overlay::{LayerEvent, OverlayLayer};

pub struct InfoPopupLayer;

impl OverlayLayer for InfoPopupLayer {
	fn name(&self) -> &'static str {
		"InfoPopup"
	}

	fn is_visible(&self, ed: &Editor) -> bool {
		ed.overlays().get::<InfoPopupStore>().is_some_and(|s| !s.is_empty())
	}

	fn layout(&self, _ed: &Editor, _screen: Rect) -> Option<Rect> {
		// Info popups are rendered by `xeno-editor-tui` scene layers.
		// This overlay layer is event-only and handles dismissal triggers.
		None
	}

	fn render(&self, _ed: &Editor, _frame: &mut Frame, _area: Rect) {
		// Rendering is handled by scene compositor layers.
	}

	fn on_event(&mut self, ed: &mut Editor, event: &LayerEvent) {
		match event {
			LayerEvent::CursorMoved { .. } | LayerEvent::ModeChanged { .. } | LayerEvent::FocusChanged { .. } => {
				ed.close_all_info_popups();
			}
			_ => {}
		}
	}
}
