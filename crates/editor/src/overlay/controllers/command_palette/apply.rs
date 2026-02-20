use super::*;

impl CommandPaletteOverlay {
	pub(super) fn selected_completion_item(ctx: &dyn OverlayContext) -> Option<CompletionItem> {
		crate::overlay::picker_engine::decision::selected_completion_item(ctx.completion_state())
	}

	pub(super) fn apply_selected_completion(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, cycle_on_exact_match: bool) -> bool {
		let Some(mut selected_item) = Self::selected_completion_item(ctx) else {
			return true;
		};

		let (input, cursor) = Self::current_input_and_cursor(ctx, session);
		let token = Self::token_context(&input, cursor);
		let replace_end = Self::effective_replace_end(&token, cursor);
		let current_replacement: String = input.chars().skip(token.start).take(replace_end.saturating_sub(token.start)).collect();

		if cycle_on_exact_match && crate::overlay::picker_engine::decision::is_exact_selection_match(&current_replacement, &selected_item) {
			let _ = self.move_selection(ctx, 1);
			if let Some(next) = Self::selected_completion_item(ctx) {
				selected_item = next;
			}
		}

		let is_dir_completion = selected_item.kind == CompletionKind::File && selected_item.insert_text.ends_with('/');
		let quoted_arg = selected_item.kind == CompletionKind::File && token.quoted.is_some();

		let mut replacement = selected_item.insert_text.clone();
		if Self::should_append_space_after_completion(&selected_item, &token, is_dir_completion, quoted_arg) {
			replacement.push(' ');
		}

		let replaced_len = replace_end.saturating_sub(token.start);
		let replacement_len = Self::char_count(&replacement);
		let delta = replacement_len as isize - replaced_len as isize;

		let (mut new_input, mut new_cursor) = Self::replace_char_range(&input, token.start, replace_end, &replacement);

		if selected_item.kind == CompletionKind::File
			&& !is_dir_completion
			&& quoted_arg
			&& let Some(close_quote_idx) = token.close_quote_idx
		{
			let close_quote_new = (close_quote_idx as isize + delta).max(0) as usize;
			let mut after_quote = close_quote_new.saturating_add(1);
			if Self::char_at(&new_input, after_quote).is_none_or(|ch| !ch.is_whitespace()) {
				new_input = Self::insert_char_at(&new_input, after_quote, ' ');
			}
			after_quote = after_quote.saturating_add(1);
			new_cursor = after_quote.min(Self::char_count(&new_input));
		}

		ctx.reset_buffer_content(session.input, &new_input);
		if let Some(buffer) = ctx.buffer_mut(session.input) {
			buffer.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		}

		self.last_input = new_input.clone();
		self.refresh_for(ctx, session, &new_input, new_cursor);
		ctx.request_redraw();
		true
	}

	pub(super) fn accept_tab_completion(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) -> bool {
		self.apply_selected_completion(ctx, session, true)
	}
}
