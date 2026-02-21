use xeno_registry::actions::{EditorCapabilities, MacroAccess, UndoAccess};

use crate::capabilities::provider::EditorCaps;

impl EditorCapabilities for EditorCaps<'_> {
	fn search(&mut self) -> &mut dyn xeno_registry::actions::SearchAccess {
		self
	}

	fn undo(&mut self) -> &mut dyn UndoAccess {
		self
	}

	fn edit(&mut self) -> &mut dyn xeno_registry::actions::EditAccess {
		self
	}

	fn motion(&mut self) -> &mut dyn xeno_registry::actions::MotionAccess {
		self
	}

	fn motion_dispatch(&mut self) -> &mut dyn xeno_registry::actions::MotionDispatchAccess {
		self
	}

	fn split_ops(&mut self) -> &mut dyn xeno_registry::actions::SplitOps {
		self
	}

	fn focus_ops(&mut self) -> &mut dyn xeno_registry::actions::FocusOps {
		self
	}

	fn viewport(&mut self) -> &mut dyn xeno_registry::actions::ViewportAccess {
		self
	}

	fn file_ops(&mut self) -> &mut dyn xeno_registry::actions::FileOpsAccess {
		self
	}

	fn jump_ops(&mut self) -> &mut dyn xeno_registry::actions::JumpAccess {
		self
	}

	fn macro_ops(&mut self) -> &mut dyn MacroAccess {
		self
	}

	fn deferred_invocations(&mut self) -> &mut dyn xeno_registry::actions::DeferredInvocationAccess {
		self
	}

	fn palette(&mut self) -> &mut dyn xeno_registry::actions::PaletteAccess {
		self
	}

	fn option_ops(&self) -> &dyn xeno_registry::actions::OptionAccess {
		self
	}

	fn overlay(&mut self) -> &mut dyn xeno_registry::actions::editor_ctx::OverlayAccess {
		self
	}

	fn open_search_prompt(&mut self, reverse: bool) {
		self.ed.open_search(reverse);
	}

	fn is_readonly(&self) -> bool {
		self.ed.buffer().is_readonly()
	}
}
