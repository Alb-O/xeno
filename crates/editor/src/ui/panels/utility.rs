use xeno_primitives::KeyCode;
use xeno_registry::actions::BindingMode;

use crate::impls::Editor;
use crate::ui::UiRequest;
use crate::ui::dock::DockSlot;
use crate::ui::ids::UTILITY_PANEL_ID;
use crate::ui::keymap::UiKeyChord;
use crate::ui::panel::{EventResult, Panel, PanelInitContext, UiEvent};

#[derive(Default)]
pub struct UtilityPanel;

impl UtilityPanel {
	pub fn whichkey_desired_height(ed: &Editor) -> Option<u16> {
		let pending_keys = ed.buffer().input.pending_keys();
		if pending_keys.is_empty() {
			return None;
		}

		let binding_mode = match ed.buffer().input.mode() {
			xeno_primitives::Mode::Normal => BindingMode::Normal,
			_ => return None,
		};

		let registry = ed.effective_keymap();
		let row_count = registry.continuations_with_kind(binding_mode, pending_keys).len();
		if row_count == 0 {
			return None;
		}

		Some((row_count as u16 + 3).clamp(4, 10))
	}
}

impl Panel for UtilityPanel {
	fn id(&self) -> &str {
		UTILITY_PANEL_ID
	}

	fn default_slot(&self) -> DockSlot {
		DockSlot::Bottom
	}

	fn on_register(&mut self, ctx: PanelInitContext<'_>) {
		ctx.keybindings
			.register_global(UiKeyChord::ctrl_char('u'), 100, vec![UiRequest::TogglePanel(UTILITY_PANEL_ID.to_string())]);
	}

	fn handle_event(&mut self, event: UiEvent, _editor: &mut Editor, focused: bool) -> EventResult {
		match event {
			UiEvent::Key(key) if focused && key.code == KeyCode::Esc => {
				EventResult::consumed().with_request(UiRequest::ClosePanel(UTILITY_PANEL_ID.to_string()))
			}
			_ => EventResult::not_consumed(),
		}
	}
}
