use super::*;

impl CommandPaletteOverlay {
	pub(super) fn update_completion_state(
		&mut self,
		ctx: &mut dyn OverlayContext,
		items: Vec<CompletionItem>,
		replace_start: usize,
		query: String,
		token_index: usize,
	) {
		let preserve_manual = self.last_token_index == Some(token_index);
		let prev_manual_label = ctx.completion_state().and_then(|state| {
			if preserve_manual && state.selection_intent == SelectionIntent::Manual {
				state.selected_idx.and_then(|idx| state.items.get(idx).map(|item| item.label.clone()))
			} else {
				None
			}
		});

		let state = ctx.completion_state_mut();
		state.show_kind = false;
		state.suppressed = false;
		state.replace_start = replace_start;
		state.query = query;
		state.scroll_offset = 0;
		state.items = items;
		state.active = !state.items.is_empty();

		if state.items.is_empty() {
			state.selected_idx = None;
			state.selection_intent = SelectionIntent::Auto;
			self.selected_label = None;
			self.last_token_index = Some(token_index);
			return;
		}

		if let Some(label) = prev_manual_label
			&& let Some(idx) = state.items.iter().position(|item| item.label == label)
		{
			state.selected_idx = Some(idx);
			state.selection_intent = SelectionIntent::Manual;
		} else {
			state.selected_idx = Some(0);
			state.selection_intent = SelectionIntent::Auto;
		}

		state.ensure_selected_visible();
		self.selected_label = state.selected_idx.and_then(|idx| state.items.get(idx).map(|item| item.label.clone()));
		self.last_token_index = Some(token_index);
	}

	pub(super) fn refresh_for(&mut self, ctx: &mut dyn OverlayContext, session: &OverlaySession, input: &str, cursor: usize) {
		let token = Self::token_context(input, cursor);
		let usage = ctx.command_usage_snapshot();
		let items = self.build_items_for_token(&token, ctx, session, &usage);
		self.update_completion_state(ctx, items, token.start, token.query, token.token_index);
	}

	pub(super) fn move_selection(&mut self, ctx: &mut dyn OverlayContext, delta: isize) -> bool {
		let state = ctx.completion_state_mut();
		if state.items.is_empty() {
			return false;
		}

		let total = state.items.len() as isize;
		let current = state.selected_idx.unwrap_or(0) as isize;
		let mut next = current + delta;
		if next < 0 {
			next = total - 1;
		} else if next >= total {
			next = 0;
		}

		state.selected_idx = Some(next as usize);
		state.selection_intent = SelectionIntent::Manual;
		state.ensure_selected_visible();
		self.selected_label = state.items.get(next as usize).map(|item| item.label.clone());
		ctx.request_redraw();
		true
	}

	pub(super) fn page_selection(&mut self, ctx: &mut dyn OverlayContext, direction: isize) -> bool {
		let state = ctx.completion_state_mut();
		if state.items.is_empty() {
			return false;
		}

		let step = CompletionState::MAX_VISIBLE as isize;
		let delta = if direction >= 0 { step } else { -step };
		let total = state.items.len();
		let current = state.selected_idx.unwrap_or(0) as isize;
		let mut next = current + delta;
		if next < 0 {
			next = 0;
		} else if next as usize >= total {
			next = total.saturating_sub(1) as isize;
		}

		state.selected_idx = Some(next as usize);
		state.selection_intent = SelectionIntent::Manual;
		state.ensure_selected_visible();
		self.selected_label = state.items.get(next as usize).map(|item| item.label.clone());
		ctx.request_redraw();
		true
	}
}
