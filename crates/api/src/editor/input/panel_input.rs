//! Panel input handling.
//!
//! Routing key and mouse events to focused panels.

use evildoer_manifest::{SplitKey, SplitMouse};

use crate::editor::Editor;

impl Editor {
	/// Handles a key event for a panel.
	pub(crate) fn handle_panel_key(
		&mut self,
		panel_id: evildoer_manifest::PanelId,
		key: SplitKey,
	) -> evildoer_manifest::SplitEventResult {
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.handle_key(key)
		} else {
			evildoer_manifest::SplitEventResult::ignored()
		}
	}

	/// Handles a mouse event for a panel.
	pub(crate) fn handle_panel_mouse(
		&mut self,
		panel_id: evildoer_manifest::PanelId,
		mouse: SplitMouse,
	) -> evildoer_manifest::SplitEventResult {
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.handle_mouse(mouse)
		} else {
			evildoer_manifest::SplitEventResult::ignored()
		}
	}
}
