use xeno_registry::actions::MacroAccess;

use crate::capabilities::provider::EditorCaps;

impl MacroAccess for EditorCaps<'_> {
	fn record(&mut self) {
		self.ed.state.core.editor.workspace.macro_state.start_recording('q');
	}

	fn stop_recording(&mut self) {
		self.ed.state.core.editor.workspace.macro_state.stop_recording();
	}

	fn play(&mut self) {
		// TODO: Requires event loop integration
	}

	fn is_recording(&self) -> bool {
		self.ed.state.core.editor.workspace.macro_state.is_recording()
	}
}
