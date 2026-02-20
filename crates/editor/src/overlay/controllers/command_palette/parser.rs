use super::*;

impl CommandPaletteOverlay {
	pub(super) fn char_count(s: &str) -> usize {
		s.chars().count()
	}

	pub(super) fn char_at(s: &str, idx: usize) -> Option<char> {
		s.chars().nth(idx)
	}

	pub(super) fn insert_char_at(s: &str, idx: usize, ch: char) -> String {
		let mut out = String::new();
		let chars: Vec<char> = s.chars().collect();
		let idx = idx.min(chars.len());
		for c in &chars[..idx] {
			out.push(*c);
		}
		out.push(ch);
		for c in &chars[idx..] {
			out.push(*c);
		}
		out
	}

	pub(super) fn replace_char_range(input: &str, start: usize, end: usize, replacement: &str) -> (String, usize) {
		crate::overlay::picker_engine::apply::replace_char_range(input, start, end, replacement)
	}

	pub(super) fn tokenize(chars: &[char]) -> Vec<Tok> {
		crate::overlay::picker_engine::parser::tokenize(chars)
	}

	pub(super) fn token_context(input: &str, cursor: usize) -> TokenCtx {
		let chars: Vec<char> = input.chars().collect();
		let len = chars.len();
		let cursor = cursor.min(len);
		let tokens = Self::tokenize(&chars);

		let cmd = tokens
			.first()
			.map(|tok| chars[tok.content_start..tok.content_end].iter().collect::<String>().to_ascii_lowercase())
			.unwrap_or_default();
		let args = tokens
			.iter()
			.skip(1)
			.map(|tok| chars[tok.content_start..tok.content_end].iter().collect::<String>())
			.collect::<Vec<_>>();

		if let Some((idx, tok)) = tokens.iter().enumerate().find(|(_, tok)| cursor >= tok.start && cursor <= tok.end) {
			let cursor_in_content = cursor.clamp(tok.content_start, tok.content_end);
			let mut start = tok.content_start;
			let mut query: String = chars[tok.content_start..cursor_in_content].iter().collect();
			let mut path_dir = None;

			if idx >= 1 && Self::command_arg_completion(&cmd, idx) == CommandArgCompletion::FilePath {
				let (dir_part, file_part) = Self::split_path_query(&query);
				start = start.saturating_add(Self::char_count(&dir_part));
				query = file_part;
				if !dir_part.is_empty() {
					path_dir = Some(dir_part);
				}
			}

			return TokenCtx {
				cmd,
				token_index: idx,
				start,
				query,
				args,
				path_dir,
				quoted: tok.quoted,
				close_quote_idx: tok.close_quote_idx,
			};
		}

		let token_index = tokens.iter().filter(|tok| tok.end <= cursor).count();
		TokenCtx {
			cmd,
			token_index,
			start: cursor,
			query: String::new(),
			args,
			path_dir: None,
			quoted: None,
			close_quote_idx: None,
		}
	}

	pub(super) fn split_path_query(query: &str) -> (String, String) {
		let slash_idx = query
			.char_indices()
			.rev()
			.find(|(_, ch)| *ch == '/' || *ch == '\\')
			.map(|(idx, ch)| idx + ch.len_utf8());
		if let Some(idx) = slash_idx {
			(query[..idx].to_string(), query[idx..].to_string())
		} else {
			(String::new(), query.to_string())
		}
	}

	pub(super) fn effective_replace_end(token: &TokenCtx, cursor: usize) -> usize {
		let picker_token = crate::overlay::picker_engine::parser::PickerToken {
			start: token.start,
			end: cursor,
			content_start: token.start,
			content_end: cursor,
			quoted: token.quoted,
			close_quote_idx: token.close_quote_idx,
		};
		crate::overlay::picker_engine::parser::effective_replace_end(&picker_token, cursor)
	}

	pub(super) fn current_input_and_cursor(ctx: &mut dyn OverlayContext, session: &OverlaySession) -> (String, usize) {
		let input = session.input_text(ctx).trim_end_matches('\n').to_string();
		let input_len = Self::char_count(&input);
		let cursor = ctx.buffer(session.input).map(|buffer| buffer.cursor).unwrap_or(input_len);
		(input, cursor.min(input_len))
	}
}
