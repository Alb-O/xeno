//! Panel input handling.
//!
//! Routing key and mouse events to focused panels.

use evildoer_registry::panels::{PanelId, SplitEventResult, SplitKey, SplitMouse};

use crate::editor::Editor;

impl Editor {
	/// Handles a key event for a panel.
	pub(crate) fn handle_panel_key(
		&mut self,
		panel_id: PanelId,
		key: SplitKey,
	) -> SplitEventResult {
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.handle_key(key)
		} else {
			SplitEventResult::ignored()
		}
	}

	/// Handles a mouse event for a panel.
	pub(crate) fn handle_panel_mouse(
		&mut self,
		panel_id: PanelId,
		mouse: SplitMouse,
	) -> SplitEventResult {
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.handle_mouse(mouse)
		} else {
			SplitEventResult::ignored()
		}
	}
}
