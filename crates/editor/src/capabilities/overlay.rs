use xeno_registry::actions::editor_ctx::{OverlayAccess, OverlayRequest};
use xeno_registry::commands::CommandError;

use crate::capabilities::provider::EditorCaps;

impl OverlayAccess for EditorCaps<'_> {
	fn overlay_request(&mut self, req: OverlayRequest) -> Result<(), CommandError> {
		// Synchronous validation for immediate error reporting
		self.ed.validate_overlay_request(&req)?;

		// Single-path dispatch via sink
		self.ed.state.runtime.effects.overlay_request(req);
		Ok(())
	}

	fn overlay_modal_is_open(&self) -> bool {
		self.ed.state.ui.overlay_system.interaction().is_open()
	}
}
