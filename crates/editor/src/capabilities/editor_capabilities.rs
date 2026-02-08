use xeno_registry::actions::{EditorCapabilities, MacroAccess, UndoAccess};

use crate::capabilities::provider::EditorCaps;

impl EditorCapabilities for EditorCaps<'_> {
	fn search(&mut self) -> Option<&mut dyn xeno_registry::actions::SearchAccess> {
		Some(self)
	}

	fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		Some(self)
	}

	fn edit(&mut self) -> Option<&mut dyn xeno_registry::actions::EditAccess> {
		Some(self)
	}

	fn motion(&mut self) -> Option<&mut dyn xeno_registry::actions::MotionAccess> {
		Some(self)
	}

	fn motion_dispatch(&mut self) -> Option<&mut dyn xeno_registry::actions::MotionDispatchAccess> {
		Some(self)
	}

	fn split_ops(&mut self) -> Option<&mut dyn xeno_registry::actions::SplitOps> {
		Some(self)
	}

	fn focus_ops(&mut self) -> Option<&mut dyn xeno_registry::actions::FocusOps> {
		Some(self)
	}

	fn viewport(&mut self) -> Option<&mut dyn xeno_registry::actions::ViewportAccess> {
		Some(self)
	}

	fn file_ops(&mut self) -> Option<&mut dyn xeno_registry::actions::FileOpsAccess> {
		Some(self)
	}

	fn jump_ops(&mut self) -> Option<&mut dyn xeno_registry::actions::JumpAccess> {
		Some(self)
	}

	fn macro_ops(&mut self) -> Option<&mut dyn MacroAccess> {
		Some(self)
	}

	fn command_queue(&mut self) -> Option<&mut dyn xeno_registry::actions::CommandQueueAccess> {
		Some(self)
	}

	fn palette(&mut self) -> Option<&mut dyn xeno_registry::actions::PaletteAccess> {
		Some(self)
	}

	fn option_ops(&self) -> Option<&dyn xeno_registry::actions::OptionAccess> {
		Some(self)
	}

	fn overlay(&mut self) -> Option<&mut dyn xeno_registry::actions::editor_ctx::OverlayAccess> {
		Some(self)
	}

	fn open_search_prompt(&mut self, reverse: bool) {
		self.ed.open_search(reverse);
	}

	fn is_readonly(&self) -> bool {
		self.ed.buffer().is_readonly()
	}
}
