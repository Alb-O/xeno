use super::*;

impl CommandPaletteOverlay {
	pub(super) fn command_resolves(command_name: &str) -> bool {
		crate::commands::find_editor_command(command_name).is_some() || xeno_registry::commands::find_command(command_name).is_some()
	}

	pub(super) fn resolve_command_name_for_commit(typed_name: &str, token_index: usize, selected_item: Option<&CompletionItem>) -> String {
		if Self::command_resolves(typed_name) {
			return typed_name.to_string();
		}

		if token_index == 0
			&& let Some(item) = selected_item
			&& item.kind == CompletionKind::Command
			&& !item.insert_text.is_empty()
			&& Self::command_resolves(&item.insert_text)
		{
			return item.insert_text.clone();
		}

		typed_name.to_string()
	}

	pub(super) fn should_promote_enter_to_tab_completion(input: &str, cursor: usize, selected_item: Option<&CompletionItem>) -> bool {
		let chars: Vec<char> = input.chars().collect();
		let tokens = Self::tokenize(&chars);
		let Some(name_tok) = tokens.first() else {
			return false;
		};
		if tokens.len() != 1 {
			return false;
		}

		let token = Self::token_context(input, cursor);
		if token.token_index != 0 {
			return false;
		}

		let typed_name: String = chars[name_tok.content_start..name_tok.content_end].iter().collect();
		let command_name = if Self::command_resolves(&typed_name) {
			typed_name
		} else {
			let Some(selected) = selected_item else {
				return false;
			};
			if selected.kind != CompletionKind::Command || selected.insert_text.is_empty() {
				return false;
			}
			if !Self::command_resolves(&selected.insert_text) {
				return false;
			}
			selected.insert_text.clone()
		};

		if !Self::command_requires_argument_for_commit(&command_name) {
			return false;
		}

		Self::command_arg_spec(&command_name, 1).map(|spec| spec.required).unwrap_or(false)
	}

	pub(super) fn command_argument_is_resolved_for_commit(command_name: &str, token_index: usize, arg: Option<&str>) -> bool {
		let Some(spec) = Self::command_arg_spec(command_name, token_index) else {
			return true;
		};

		if arg.is_none() {
			return !spec.required;
		}
		let value = arg.expect("arg presence checked");
		if value.is_empty() {
			return !spec.required;
		}

		match CommandArgCompletion::from_palette_kind(spec.kind) {
			CommandArgCompletion::Theme => xeno_registry::themes::get_theme(value).is_some(),
			CommandArgCompletion::Snippet => !value.starts_with('@') || xeno_registry::snippets::find_snippet(value).is_some(),
			CommandArgCompletion::CommandName => Self::command_resolves(value),
			CommandArgCompletion::OptionKey => xeno_registry::options::find(value).is_some(),
			CommandArgCompletion::OptionValue => true,
			CommandArgCompletion::FilePath | CommandArgCompletion::Buffer | CommandArgCompletion::FreeText => true,
			CommandArgCompletion::None => true,
		}
	}

	pub(super) fn should_apply_selected_argument_on_commit(input: &str, cursor: usize, command_name: &str, selected_item: Option<&CompletionItem>) -> bool {
		if !Self::command_requires_argument_for_commit(command_name) {
			return false;
		}

		let token = Self::token_context(input, cursor);
		if token.token_index < 1 {
			return false;
		}

		let Some(selected) = selected_item else {
			return false;
		};

		let completion_kind = Self::command_arg_completion(command_name, token.token_index);
		let Some(expected_kind) = completion_kind.completion_kind() else {
			return false;
		};
		if selected.kind != expected_kind || selected.insert_text.is_empty() {
			return false;
		}

		let chars: Vec<char> = input.chars().collect();
		let tokens = Self::tokenize(&chars);
		let arg = tokens
			.get(token.token_index)
			.map(|tok| chars[tok.content_start..tok.content_end].iter().collect::<String>());

		!Self::command_argument_is_resolved_for_commit(command_name, token.token_index, arg.as_deref())
	}

	pub(super) fn enter_commit_decision(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) -> CommitDecision {
		let (input, cursor) = Self::current_input_and_cursor(ctx, session);
		let selected_item = Self::selected_completion_item(ctx);
		if Self::should_promote_enter_to_tab_completion(&input, cursor, selected_item.as_ref()) {
			return CommitDecision::ApplySelectionThenStay;
		}

		CommitDecision::CommitTyped
	}

	pub(super) fn picker_action_for_key(key: Key) -> Option<PickerAction> {
		match key.code {
			KeyCode::Enter => Some(PickerAction::Commit(CommitDecision::CommitTyped)),
			KeyCode::Up => Some(PickerAction::MoveSelection { delta: -1 }),
			KeyCode::Down => Some(PickerAction::MoveSelection { delta: 1 }),
			KeyCode::PageUp => Some(PickerAction::PageSelection { direction: -1 }),
			KeyCode::PageDown => Some(PickerAction::PageSelection { direction: 1 }),
			KeyCode::Char('n') if key.modifiers.ctrl => Some(PickerAction::MoveSelection { delta: 1 }),
			KeyCode::Char('p') if key.modifiers.ctrl => Some(PickerAction::MoveSelection { delta: -1 }),
			KeyCode::Tab => Some(PickerAction::ApplySelection),
			_ => None,
		}
	}

	pub(super) fn handle_picker_action(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, action: PickerAction) -> bool {
		match action {
			PickerAction::MoveSelection { delta } => self.move_selection(ctx, delta),
			PickerAction::PageSelection { direction } => self.page_selection(ctx, direction),
			PickerAction::ApplySelection => self.accept_tab_completion(ctx, session),
			PickerAction::Commit(_) => match self.enter_commit_decision(ctx, session) {
				CommitDecision::CommitTyped => false,
				CommitDecision::ApplySelectionThenStay => self.apply_selected_completion(ctx, session, false),
			},
		}
	}
}
