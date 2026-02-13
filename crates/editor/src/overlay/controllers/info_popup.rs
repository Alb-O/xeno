use crate::Editor;
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

	fn on_event(&mut self, ed: &mut Editor, event: &LayerEvent) {
		match event {
			LayerEvent::CursorMoved { .. } | LayerEvent::ModeChanged { .. } | LayerEvent::FocusChanged { .. } => {
				ed.close_all_info_popups();
			}
			_ => {}
		}
	}
}
