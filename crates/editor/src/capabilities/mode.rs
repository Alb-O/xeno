use xeno_primitives::Mode;
use xeno_registry::ModeAccess;
use xeno_registry::notifications::keys;

use crate::capabilities::provider::EditorCaps;
use crate::overlay::LayerEvent;

impl ModeAccess for EditorCaps<'_> {
	fn mode(&self) -> Mode {
		self.ed.buffer().input.mode()
	}

	fn set_mode(&mut self, mode: Mode) {
		if matches!(mode, Mode::Insert) && self.ed.buffer().is_readonly() {
			self.ed.notify(keys::BUFFER_READONLY);
			return;
		}
		#[cfg(feature = "lsp")]
		if matches!(mode, Mode::Insert) {
			self.ed
				.overlays_mut()
				.get_or_default::<crate::completion::CompletionState>()
				.suppressed = false;
		}
		let view = self.ed.focused_view();
		self.ed.buffer_mut().input.set_mode(mode.clone());
		self.ed
			.state
			.effects
			.push_layer_event(LayerEvent::ModeChanged { view, mode });
	}
}
