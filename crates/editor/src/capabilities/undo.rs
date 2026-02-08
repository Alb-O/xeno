use xeno_registry::actions::UndoAccess;

use crate::capabilities::provider::EditorCaps;

impl UndoAccess for EditorCaps<'_> {
	fn undo(&mut self) {
		self.ed.undo();
	}

	fn redo(&mut self) {
		self.ed.redo();
	}

	fn can_undo(&self) -> bool {
		self.ed.state.core.undo_manager.can_undo()
	}

	fn can_redo(&self) -> bool {
		self.ed.state.core.undo_manager.can_redo()
	}
}
