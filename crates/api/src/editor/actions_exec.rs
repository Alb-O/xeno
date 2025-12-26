use ropey::Rope;
use tome_base::Selection;
use tome_manifest::{ActionArgs, ActionContext, ActionResult, find_action};

use crate::editor::Editor;

impl Editor {
	pub(crate) fn execute_action(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
	) -> bool {
		let action = match find_action(name) {
			Some(a) => a,
			None => {
				self.notify("error", format!("Unknown action: {}", name));
				return false;
			}
		};

		// Check required capabilities
		{
			use tome_manifest::EditorContext;
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(action.required_caps) {
				self.notify("error", e.to_string());
				return false;
			}
		}

		// When terminal is focused, use a dummy context for workspace-level actions
		// (window mode actions like split_horizontal, buffer_next, etc. don't use the context)
		let result = if self.is_terminal_focused() {
			let dummy_rope = Rope::new();
			let dummy_selection = Selection::point(0);
			let ctx = ActionContext {
				text: dummy_rope.slice(..),
				cursor: 0,
				selection: &dummy_selection,
				count,
				extend,
				register,
				args: ActionArgs::default(),
			};
			(action.handler)(&ctx)
		} else {
			let ctx = ActionContext {
				text: self.buffer().doc.slice(..),
				cursor: self.buffer().cursor,
				selection: &self.buffer().selection,
				count,
				extend,
				register,
				args: ActionArgs::default(),
			};
			(action.handler)(&ctx)
		};

		self.apply_action_result(result, extend)
	}

	pub(crate) fn execute_action_with_char(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: char,
	) -> bool {
		let action = match find_action(name) {
			Some(a) => a,
			None => {
				self.notify("error", format!("Unknown action: {}", name));
				return false;
			}
		};

		// Check required capabilities
		{
			use tome_manifest::EditorContext;
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(action.required_caps) {
				self.notify("error", e.to_string());
				return false;
			}
		}

		// When terminal is focused, use a dummy context for workspace-level actions
		let result = if self.is_terminal_focused() {
			let dummy_rope = Rope::new();
			let dummy_selection = Selection::point(0);
			let ctx = ActionContext {
				text: dummy_rope.slice(..),
				cursor: 0,
				selection: &dummy_selection,
				count,
				extend,
				register,
				args: ActionArgs {
					char: Some(char_arg),
					string: None,
				},
			};
			(action.handler)(&ctx)
		} else {
			let ctx = ActionContext {
				text: self.buffer().doc.slice(..),
				cursor: self.buffer().cursor,
				selection: &self.buffer().selection,
				count,
				extend,
				register,
				args: ActionArgs {
					char: Some(char_arg),
					string: None,
				},
			};
			(action.handler)(&ctx)
		};

		self.apply_action_result(result, extend)
	}

	pub(crate) fn apply_action_result(&mut self, result: ActionResult, extend: bool) -> bool {
		use tome_manifest::{EditorContext, dispatch_result};
		let mut ctx = EditorContext::new(self);
		dispatch_result(&result, &mut ctx, extend)
	}
}
