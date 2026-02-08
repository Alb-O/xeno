use xeno_registry::actions::PaletteAccess;
use xeno_registry::actions::editor_ctx::{OverlayCloseReason, OverlayRequest};

use crate::capabilities::provider::EditorCaps;

impl PaletteAccess for EditorCaps<'_> {
	fn open_palette(&mut self) {
		self.ed
			.state
			.effects
			.overlay_request(OverlayRequest::OpenModal {
				kind: "command_palette",
				args: vec![],
			});
	}

	fn close_palette(&mut self) {
		self.ed
			.state
			.effects
			.overlay_request(OverlayRequest::CloseModal {
				reason: OverlayCloseReason::Cancel,
			});
	}

	fn execute_palette(&mut self) {
		self.ed
			.state
			.effects
			.overlay_request(OverlayRequest::CloseModal {
				reason: OverlayCloseReason::Commit,
			});
	}

	fn palette_is_open(&self) -> bool {
		self.ed.state.overlay_system.interaction.is_open()
	}
}
