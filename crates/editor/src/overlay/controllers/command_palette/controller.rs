use super::*;

impl Default for CommandPaletteOverlay {
	fn default() -> Self {
		Self::new()
	}
}

impl CommandPaletteOverlay {
	pub fn new() -> Self {
		Self {
			last_input: String::new(),
			selected_label: None,
			last_token_index: None,
			file_cache: None,
		}
	}
}

impl OverlayController for CommandPaletteOverlay {
	fn name(&self) -> &'static str {
		"CommandPalette"
	}

	fn kind(&self) -> crate::overlay::OverlayControllerKind {
		crate::overlay::OverlayControllerKind::CommandPalette
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		OverlayUiSpec {
			title: None,
			gutter: GutterSelector::Prompt('>'),
			rect: RectPolicy::TopCenter {
				width_percent: 100,
				max_width: u16::MAX,
				min_width: 1,
				y_frac: (1, 1),
				height: 1,
			},
			style: crate::overlay::docked_prompt_style(),
			windows: vec![],
		}
	}

	fn on_open(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) {
		if let Some(buffer) = ctx.buffer_mut(session.input) {
			let opt = xeno_registry::OPTIONS
				.get_key(&opt_keys::CURSORLINE.untyped())
				.expect("cursorline option missing from registry");
			buffer.local_options.set(opt, OptionValue::Bool(false));
		}

		let (input, cursor) = Self::current_input_and_cursor(ctx, session);
		self.last_input = input.clone();
		self.refresh_for(ctx, session, &input, cursor);
		ctx.request_redraw();
	}

	fn on_input_changed(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, text: &str) {
		let input = text.trim_end_matches('\n').to_string();
		let cursor = ctx
			.buffer(session.input)
			.map(|buffer| buffer.cursor)
			.unwrap_or_else(|| Self::char_count(&input));
		if input == self.last_input {
			return;
		}
		self.last_input = input.clone();
		self.refresh_for(ctx, session, &input, cursor.min(Self::char_count(&input)));
		ctx.request_redraw();
	}

	fn on_key(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, key: Key) -> bool {
		let Some(action) = Self::picker_action_for_key(key) else {
			return false;
		};
		self.handle_picker_action(ctx, session, action)
	}

	fn on_commit<'a>(&'a mut self, ctx: &'a mut dyn OverlayContext, session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let mut input = session.input_text(ctx).trim_end_matches('\n').to_string();

		if !input.trim().is_empty() {
			let mut chars: Vec<char> = input.chars().collect();
			let mut tokens = Self::tokenize(&chars);
			if let Some(name_tok) = tokens.first() {
				let typed_name: String = chars[name_tok.content_start..name_tok.content_end].iter().collect();
				let cursor = Self::char_count(&input);
				let token = Self::token_context(&input, cursor);
				let selected_item = Self::selected_completion_item(ctx);
				let mut command_name = Self::resolve_command_name_for_commit(&typed_name, token.token_index, selected_item.as_ref());

				if Self::should_apply_selected_argument_on_commit(&input, cursor, &command_name, selected_item.as_ref()) {
					let _ = self.apply_selected_completion(ctx, session, false);
					input = session.input_text(ctx).trim_end_matches('\n').to_string();
					chars = input.chars().collect();
					tokens = Self::tokenize(&chars);
					if let Some(updated_name_tok) = tokens.first() {
						let updated_typed_name: String = chars[updated_name_tok.content_start..updated_name_tok.content_end].iter().collect();
						let updated_token = Self::token_context(&input, Self::char_count(&input));
						let updated_selected = Self::selected_completion_item(ctx);
						command_name = Self::resolve_command_name_for_commit(&updated_typed_name, updated_token.token_index, updated_selected.as_ref());
					}
				}

				let args: Vec<String> = tokens
					.iter()
					.skip(1)
					.map(|tok| chars[tok.content_start..tok.content_end].iter().collect())
					.collect();

				if let Some(cmd) = crate::commands::find_editor_command(&command_name) {
					ctx.queue_invocation(xeno_registry::actions::DeferredInvocationRequest::editor_command(cmd.name.to_string(), args));
					ctx.record_command_usage(cmd.name);
				} else if let Some(cmd) = xeno_registry::commands::find_command(&command_name) {
					ctx.queue_invocation(xeno_registry::actions::DeferredInvocationRequest::command(cmd.name_str().to_string(), args));
					ctx.record_command_usage(cmd.name_str());
				} else {
					ctx.notify(keys::unknown_command(&command_name));
				}
			}
		}

		Box::pin(async {})
	}

	fn on_close(&mut self, ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _reason: CloseReason) {
		ctx.clear_completion_state();
		self.last_input.clear();
		self.selected_label = None;
		self.last_token_index = None;
		self.file_cache = None;
		ctx.request_redraw();
	}
}
