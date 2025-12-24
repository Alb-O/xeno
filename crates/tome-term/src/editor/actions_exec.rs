use tome_core::registry::{
	ActionArgs, ActionContext, ActionResult, CommandContext, CommandOutcome, find_action,
	find_command,
};

use crate::editor::Editor;

impl Editor {
	pub async fn execute_ex_command(&mut self, input: &str) -> bool {
		let input = input.trim();
		let input = input.strip_prefix(':').unwrap_or(input);
		self.execute_command_line(input).await
	}

	pub(crate) async fn execute_command_line(&mut self, input: &str) -> bool {
		let trimmed = input.trim();
		if trimmed.is_empty() {
			return false;
		}

		let mut parts = trimmed.split_whitespace();
		let name = match parts.next() {
			Some(n) => n,
			None => return false,
		};

		let arg_strings: Vec<String> = parts.map(|s| s.to_string()).collect();
		let args: Vec<&str> = arg_strings.iter().map(|s| s.as_str()).collect();

		let cmd = match find_command(name) {
			Some(cmd) => cmd,
			None => {
				self.show_error(format!("Unknown command: {}", name));
				return false;
			}
		};

		// Check required capabilities before creating ctx
		{
			use tome_core::registry::EditorContext;
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(cmd.required_caps) {
				self.show_error(e.to_string());
				return false;
			}
		}

		let result = {
			let mut ctx = CommandContext {
				editor: self,
				args: &args,
				count: 1,
				register: None,
				user_data: cmd.user_data,
			};

			(cmd.handler)(&mut ctx).await
		};

		match result {
			Ok(CommandOutcome::Ok) => false,
			Ok(CommandOutcome::Quit) => true,
			Ok(CommandOutcome::ForceQuit) => true,
			Err(e) => {
				self.show_error(e.to_string());
				false
			}
		}
	}

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
				self.show_error(format!("Unknown action: {}", name));
				return false;
			}
		};

		// Check required capabilities
		{
			use tome_core::registry::EditorContext;
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(action.required_caps) {
				self.show_error(e.to_string());
				return false;
			}
		}

		let ctx = ActionContext {
			text: self.doc.slice(..),
			cursor: self.cursor,
			selection: &self.selection,
			count,
			extend,
			register,
			args: ActionArgs::default(),
		};

		let result = (action.handler)(&ctx);
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
				self.show_error(format!("Unknown action: {}", name));
				return false;
			}
		};

		// Check required capabilities
		{
			use tome_core::registry::EditorContext;
			let mut e_ctx = EditorContext::new(self);
			if let Err(e) = e_ctx.check_all_capabilities(action.required_caps) {
				self.show_error(e.to_string());
				return false;
			}
		}

		let ctx = ActionContext {
			text: self.doc.slice(..),
			cursor: self.cursor,
			selection: &self.selection,
			count,
			extend,
			register,
			args: ActionArgs {
				char: Some(char_arg),
				string: None,
			},
		};

		let result = (action.handler)(&ctx);
		self.apply_action_result(result, extend)
	}

	pub(crate) fn apply_action_result(&mut self, result: ActionResult, extend: bool) -> bool {
		use tome_core::registry::{EditorContext, dispatch_result};
		let mut ctx = EditorContext::new(self);
		dispatch_result(&result, &mut ctx, extend)
	}
}
