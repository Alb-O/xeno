use evildoer_base::Selection;
use evildoer_registry::actions::find_action;
use evildoer_registry::{
	ActionArgs, ActionContext, ActionResult, EditorContext, HookContext, HookEventData,
	dispatch_result, emit_sync_with as emit_hook_sync_with,
};
use ropey::Rope;
use tracing::{debug, info_span};

use crate::editor::Editor;

/// Returns the variant name of an action result for hook events.
fn action_result_variant(result: &ActionResult) -> &'static str {
	match result {
		ActionResult::Ok => "Ok",
		ActionResult::Quit => "Quit",
		ActionResult::ForceQuit => "ForceQuit",
		ActionResult::Error(_) => "Error",
		ActionResult::ForceRedraw => "ForceRedraw",
		ActionResult::SplitHorizontal => "SplitHorizontal",
		ActionResult::SplitVertical => "SplitVertical",
		ActionResult::TogglePanel(_) => "TogglePanel",
		ActionResult::BufferNext => "BufferNext",
		ActionResult::BufferPrev => "BufferPrev",
		ActionResult::CloseSplit => "CloseSplit",
		ActionResult::CloseOtherBuffers => "CloseOtherBuffers",
		ActionResult::FocusLeft => "FocusLeft",
		ActionResult::FocusRight => "FocusRight",
		ActionResult::FocusUp => "FocusUp",
		ActionResult::FocusDown => "FocusDown",
		ActionResult::ModeChange(_) => "ModeChange",
		ActionResult::CursorMove(_) => "CursorMove",
		ActionResult::ScreenMotion { .. } => "ScreenMotion",
		ActionResult::Motion(_) => "Motion",
		ActionResult::InsertWithMotion(_) => "InsertWithMotion",
		ActionResult::Edit(_) => "Edit",
		ActionResult::Pending(_) => "Pending",
		ActionResult::SearchNext { .. } => "SearchNext",
		ActionResult::SearchPrev { .. } => "SearchPrev",
		ActionResult::UseSelectionAsSearch => "UseSelectionAsSearch",
		ActionResult::Command { .. } => "Command",
	}
}

impl Editor {
	/// Executes a named action with the given count and options.
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
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(action.required_caps) {
				self.notify("error", e.to_string());
				return false;
			}
		}

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::ActionPre {
					action_id: action.id,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);

		let span = info_span!(
			"action",
			name = action.name,
			id = action.id,
			count = count,
			extend = extend,
		);
		let _guard = span.enter();

		// When a panel captures input, use a dummy context for workspace-level actions
		// (window mode actions like split_horizontal, buffer_next, etc. don't use the context)
		let result = if self
			.focused_panel_def()
			.is_some_and(|panel| panel.captures_input)
		{
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
			let result = (action.handler)(&ctx);

			// Reject text-editing results when terminal is focused
			if !result.is_terminal_safe() {
				self.notify("warn", "Action not available in terminal");
				return false;
			}
			result
		} else {
			self.buffer_mut().ensure_valid_selection();
			let (content, cursor, selection) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				(doc.content.clone(), buffer.cursor, buffer.selection.clone())
			};
			let ctx = ActionContext {
				text: content.slice(..),
				cursor,
				selection: &selection,
				count,
				extend,
				register,
				args: ActionArgs::default(),
			};
			(action.handler)(&ctx)
		};

		debug!(result = ?result, "Action completed");
		self.apply_action_result(action.id, result, extend)
	}

	/// Executes an action that requires an additional character argument.
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
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(action.required_caps) {
				self.notify("error", e.to_string());
				return false;
			}
		}

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::ActionPre {
					action_id: action.id,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);

		let span = info_span!(
			"action",
			name = action.name,
			id = action.id,
			count = count,
			extend = extend,
			char_arg = %char_arg,
		);
		let _guard = span.enter();

		// When a panel captures input, use a dummy context for workspace-level actions
		let result = if self
			.focused_panel_def()
			.is_some_and(|panel| panel.captures_input)
		{
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
			let result = (action.handler)(&ctx);

			// Reject text-editing results when terminal is focused
			if !result.is_terminal_safe() {
				self.notify("warn", "Action not available in terminal");
				return false;
			}
			result
		} else {
			self.buffer_mut().ensure_valid_selection();
			let (content, cursor, selection) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				(doc.content.clone(), buffer.cursor, buffer.selection.clone())
			};
			let ctx = ActionContext {
				text: content.slice(..),
				cursor,
				selection: &selection,
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

		debug!(result = ?result, "Action completed");
		self.apply_action_result(action.id, result, extend)
	}

	/// Dispatches an action result to handlers and emits post-action hook.
	pub(crate) fn apply_action_result(
		&mut self,
		action_id: &'static str,
		result: ActionResult,
		extend: bool,
	) -> bool {
		let mut ctx = EditorContext::new(self);
		let result_variant = action_result_variant(&result);
		let should_quit = dispatch_result(&result, &mut ctx, extend);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::ActionPost {
					action_id,
					result_variant,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
		should_quit
	}
}
