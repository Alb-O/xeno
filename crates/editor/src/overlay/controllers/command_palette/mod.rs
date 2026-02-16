//! Command palette overlay controller with command and path completion.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use xeno_primitives::{Key, KeyCode, Selection};
use xeno_registry::commands::COMMANDS;
use xeno_registry::notifications::keys;
use xeno_registry::options::{OptionValue, keys as opt_keys};
use xeno_registry::snippets::SNIPPETS;
use xeno_registry::themes::{THEMES, ThemeVariant};

use crate::completion::{CompletionItem, CompletionKind, CompletionState, SelectionIntent};
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy};
use crate::window::GutterSelector;

#[derive(Debug, Clone)]
struct TokenCtx {
	cmd: String,
	token_index: usize,
	start: usize,
	query: String,
	path_dir: Option<String>,
	quoted: Option<char>,
	close_quote_idx: Option<usize>,
}

#[derive(Debug, Clone)]
struct Tok {
	start: usize,
	end: usize,
	content_start: usize,
	content_end: usize,
	quoted: Option<char>,
	close_quote_idx: Option<usize>,
}

pub struct CommandPaletteOverlay {
	last_input: String,
	selected_label: Option<String>,
	last_token_index: Option<usize>,
	file_cache: Option<(PathBuf, Vec<(String, bool)>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandArgCompletion {
	None,
	File,
	Snippet,
	Theme,
}

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

	fn char_count(s: &str) -> usize {
		s.chars().count()
	}

	fn char_at(s: &str, idx: usize) -> Option<char> {
		s.chars().nth(idx)
	}

	fn insert_char_at(s: &str, idx: usize, ch: char) -> String {
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

	fn replace_char_range(input: &str, start: usize, end: usize, replacement: &str) -> (String, usize) {
		let chars: Vec<char> = input.chars().collect();
		let start = start.min(chars.len());
		let end = end.min(chars.len()).max(start);

		let mut out = String::new();
		for ch in &chars[..start] {
			out.push(*ch);
		}
		out.push_str(replacement);
		for ch in &chars[end..] {
			out.push(*ch);
		}

		let cursor = start + replacement.chars().count();
		(out, cursor)
	}

	fn tokenize(chars: &[char]) -> Vec<Tok> {
		let mut out = Vec::new();
		let mut i = 0usize;
		while i < chars.len() {
			while i < chars.len() && chars[i].is_whitespace() {
				i += 1;
			}
			if i >= chars.len() {
				break;
			}

			let start = i;
			if chars[i] == '"' || chars[i] == '\'' {
				let quote = chars[i];
				i += 1;
				let content_start = i;
				while i < chars.len() && chars[i] != quote {
					i += 1;
				}
				let content_end = i.min(chars.len());
				let close_quote_idx = if i < chars.len() && chars[i] == quote { Some(i) } else { None };
				if close_quote_idx.is_some() {
					i += 1;
				}
				out.push(Tok {
					start,
					end: i,
					content_start,
					content_end,
					quoted: Some(quote),
					close_quote_idx,
				});
			} else {
				let content_start = i;
				while i < chars.len() && !chars[i].is_whitespace() {
					i += 1;
				}
				out.push(Tok {
					start,
					end: i,
					content_start,
					content_end: i,
					quoted: None,
					close_quote_idx: None,
				});
			}
		}
		out
	}

	fn token_context(input: &str, cursor: usize) -> TokenCtx {
		let chars: Vec<char> = input.chars().collect();
		let len = chars.len();
		let cursor = cursor.min(len);
		let tokens = Self::tokenize(&chars);

		let cmd = tokens
			.first()
			.map(|tok| chars[tok.content_start..tok.content_end].iter().collect::<String>().to_ascii_lowercase())
			.unwrap_or_default();

		if let Some((idx, tok)) = tokens.iter().enumerate().find(|(_, tok)| cursor >= tok.start && cursor <= tok.end) {
			let cursor_in_content = cursor.clamp(tok.content_start, tok.content_end);
			let mut start = tok.content_start;
			let mut query: String = chars[tok.content_start..cursor_in_content].iter().collect();
			let mut path_dir = None;

			if idx >= 1 && Self::command_arg_completion(&cmd) == CommandArgCompletion::File {
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
			path_dir: None,
			quoted: None,
			close_quote_idx: None,
		}
	}

	fn build_command_items(query: &str, usage: &crate::completion::CommandUsageSnapshot) -> Vec<CompletionItem> {
		let query = query.trim();
		let mut scored: Vec<(bool, i32, CompletionItem)> = COMMANDS
			.snapshot_guard()
			.iter_refs()
			.filter_map(|cmd| {
				let name = cmd.name_str();
				let description = cmd.description_str();
				let mut best_score = i32::MIN;
				let mut exact_alias_match = false;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 220;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}

				for alias in cmd.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 80);
						if !alias.eq_ignore_ascii_case(name) && alias.eq_ignore_ascii_case(query) {
							exact_alias_match = true;
						}
					}
				}

				if let Some((score, _, _)) = crate::completion::frizbee_match(query, description) {
					best_score = best_score.max(score as i32 - 120);
				}

				if query.is_empty() {
					best_score = 0;
				}

				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				let right = cmd
					.keys_resolved()
					.iter()
					.find(|alias| **alias != name && alias.len() <= 8)
					.map(|alias| alias.to_string());

				let count = usage.count(name);
				let frequency_bonus = if count == 0 { 0 } else { (31 - (count + 1).leading_zeros()) as i32 * 40 };
				let recency_bonus = if query.chars().count() <= 2 {
					usage.recent_rank(name).map_or(0, |rank| (120i32 - (rank as i32 * 12)).max(0))
				} else {
					0
				};

				Some((
					exact_alias_match,
					best_score + frequency_bonus + recency_bonus,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some(description.to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices,
						right,
					},
				))
			})
			.collect();

		if !scored.iter().any(|(_, _, item)| item.label == "files") {
			let mut best_score = i32::MIN;
			let mut exact_alias_match = false;
			let mut match_indices = None;

			if let Some((score, _, indices)) = crate::completion::frizbee_match(query, "files") {
				best_score = score as i32 + 220;
				if !indices.is_empty() {
					match_indices = Some(indices);
				}
			}

			if let Some((score, _, _)) = crate::completion::frizbee_match(query, "fp") {
				best_score = best_score.max(score as i32 + 80);
				if query.eq_ignore_ascii_case("fp") {
					exact_alias_match = true;
				}
			}

			if let Some((score, _, _)) = crate::completion::frizbee_match(query, "Open file picker") {
				best_score = best_score.max(score as i32 - 120);
			}

			if query.is_empty() {
				best_score = 0;
			}

			if query.is_empty() || best_score != i32::MIN {
				let count = usage.count("files");
				let frequency_bonus = if count == 0 { 0 } else { (31 - (count + 1).leading_zeros()) as i32 * 40 };
				let recency_bonus = if query.chars().count() <= 2 {
					usage.recent_rank("files").map_or(0, |rank| (120i32 - (rank as i32 * 12)).max(0))
				} else {
					0
				};

				scored.push((
					exact_alias_match,
					best_score + frequency_bonus + recency_bonus,
					CompletionItem {
						label: "files".to_string(),
						insert_text: "files".to_string(),
						detail: Some("Open file picker".to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices,
						right: Some("fp".to_string()),
					},
				));
			}
		}

		if query.is_empty() {
			scored.sort_by(|(_, score_a, item_a), (_, score_b, item_b)| {
				let recent_a = usage.recent_rank(&item_a.label).unwrap_or(usize::MAX);
				let recent_b = usage.recent_rank(&item_b.label).unwrap_or(usize::MAX);
				recent_a
					.cmp(&recent_b)
					.then_with(|| score_b.cmp(score_a))
					.then_with(|| item_a.label.cmp(&item_b.label))
			});
		} else {
			scored.sort_by(|(exact_a, score_a, item_a), (exact_b, score_b, item_b)| {
				exact_b
					.cmp(exact_a)
					.then_with(|| score_b.cmp(score_a))
					.then_with(|| item_a.label.cmp(&item_b.label))
			});
		}

		scored.into_iter().map(|(_, _, item)| item).collect()
	}

	fn build_theme_items(query: &str) -> Vec<CompletionItem> {
		let query = query.trim();
		let mut scored: Vec<(i32, CompletionItem)> = THEMES
			.snapshot_guard()
			.iter_refs()
			.filter_map(|theme| {
				let name = theme.name_str();
				let mut best_score = i32::MIN;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 200;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}

				for alias in theme.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 70);
					}
				}

				if query.is_empty() {
					best_score = 0;
				}

				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				let variant = match theme.variant {
					ThemeVariant::Dark => "dark",
					ThemeVariant::Light => "light",
				};

				Some((
					best_score,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some(format!("{variant} theme")),
						filter_text: None,
						kind: CompletionKind::Theme,
						match_indices,
						right: Some(variant.to_string()),
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		scored.into_iter().map(|(_, item)| item).collect()
	}

	fn command_arg_completion(command_name: &str) -> CommandArgCompletion {
		let canonical = xeno_registry::commands::find_command(command_name)
			.map(|cmd| cmd.name_str().to_ascii_lowercase())
			.unwrap_or_else(|| command_name.to_ascii_lowercase());
		match canonical.as_str() {
			"open" | "edit" | "cd" => CommandArgCompletion::File,
			"snippet" => CommandArgCompletion::Snippet,
			"theme" | "colorscheme" => CommandArgCompletion::Theme,
			_ => CommandArgCompletion::None,
		}
	}

	fn command_supports_argument_completion(command_name: &str) -> bool {
		Self::command_arg_completion(command_name) != CommandArgCompletion::None
	}

	fn should_append_space_after_completion(selected: &CompletionItem, token: &TokenCtx, is_dir_completion: bool, quoted_arg: bool) -> bool {
		match selected.kind {
			CompletionKind::Command => {
				if token.token_index == 0 {
					Self::command_supports_argument_completion(&selected.insert_text)
				} else {
					true
				}
			}
			CompletionKind::File => !is_dir_completion && !quoted_arg,
			CompletionKind::Snippet | CompletionKind::Theme => true,
			CompletionKind::Buffer => false,
		}
	}

	fn build_snippet_items(query: &str) -> Vec<CompletionItem> {
		let query = query.trim();
		let query = query.strip_prefix('@').unwrap_or(query);
		let mut scored: Vec<(i32, CompletionItem)> = SNIPPETS
			.snapshot_guard()
			.iter_refs()
			.filter_map(|snippet| {
				let name = snippet.name_str();
				let label = format!("@{name}");
				let mut best_score = i32::MIN;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 220;
					if !indices.is_empty() {
						match_indices = Some(indices.into_iter().map(|idx| idx.saturating_add(1)).collect());
					}
				}

				for alias in snippet.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 80);
					}
				}

				if query.is_empty() {
					best_score = 0;
				}

				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				Some((
					best_score,
					CompletionItem {
						label: label.clone(),
						insert_text: label,
						detail: Some(snippet.description_str().to_string()),
						filter_text: None,
						kind: CompletionKind::Snippet,
						match_indices,
						right: None,
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		scored.into_iter().map(|(_, item)| item).collect()
	}

	fn file_completion_base_dir(ctx: &dyn OverlayContext, session: &OverlaySession) -> PathBuf {
		ctx.buffer(session.origin_view)
			.and_then(|buffer| buffer.path())
			.and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
			.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
	}

	fn split_path_query(query: &str) -> (String, String) {
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

	fn cached_dir_entries(&mut self, dir_path: &Path) -> Vec<(String, bool)> {
		if let Some((cached_path, cached_entries)) = &self.file_cache
			&& cached_path == dir_path
		{
			return cached_entries.clone();
		}

		let entries = fs::read_dir(dir_path)
			.ok()
			.into_iter()
			.flatten()
			.flatten()
			.take(200)
			.map(|entry| {
				let label = entry.file_name().to_string_lossy().to_string();
				let is_dir = entry.file_type().ok().is_some_and(|ft| ft.is_dir());
				(label, is_dir)
			})
			.collect::<Vec<_>>();

		self.file_cache = Some((dir_path.to_path_buf(), entries.clone()));
		entries
	}

	fn build_file_items(&mut self, query: &str, dir_part: Option<&str>, ctx: &dyn OverlayContext, session: &OverlaySession) -> Vec<CompletionItem> {
		let base_dir = Self::file_completion_base_dir(ctx, session);
		let dir_path = if let Some(dir_part) = dir_part {
			let part = PathBuf::from(dir_part);
			if part.is_absolute() { part } else { base_dir.join(part) }
		} else {
			base_dir
		};

		let mut scored = Vec::new();
		for (label, is_dir) in self.cached_dir_entries(&dir_path) {
			if !query.starts_with('.') && label.starts_with('.') {
				continue;
			}

			let Some((score, _, indices)) = crate::completion::frizbee_match(query, &label) else {
				continue;
			};

			let insert_text = if is_dir { format!("{label}/") } else { label.clone() };

			scored.push((
				score as i32 + if is_dir { 40 } else { 0 },
				CompletionItem {
					label: insert_text.clone(),
					insert_text,
					detail: Some(if is_dir { "directory".into() } else { "file".into() }),
					filter_text: None,
					kind: CompletionKind::File,
					match_indices: if indices.is_empty() { None } else { Some(indices) },
					right: Some(if is_dir { "dir".into() } else { "file".into() }),
				},
			));
		}

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		scored.into_iter().map(|(_, item)| item).collect()
	}

	fn build_items_for_token(
		&mut self,
		token: &TokenCtx,
		ctx: &dyn OverlayContext,
		session: &OverlaySession,
		usage: &crate::completion::CommandUsageSnapshot,
	) -> Vec<CompletionItem> {
		if token.token_index == 0 {
			return Self::build_command_items(&token.query, usage);
		}

		match Self::command_arg_completion(&token.cmd) {
			CommandArgCompletion::Theme if token.token_index == 1 => {
				return Self::build_theme_items(&token.query);
			}
			CommandArgCompletion::Snippet if token.token_index == 1 => {
				let query = token.query.trim_start();
				if !query.starts_with('@') {
					return Vec::new();
				}
				return Self::build_snippet_items(query);
			}
			CommandArgCompletion::File if token.token_index >= 1 => {
				return self.build_file_items(&token.query, token.path_dir.as_deref(), ctx, session);
			}
			CommandArgCompletion::None | CommandArgCompletion::Snippet | CommandArgCompletion::Theme | CommandArgCompletion::File => {}
		}

		Vec::new()
	}

	fn current_input_and_cursor(ctx: &mut dyn OverlayContext, session: &OverlaySession) -> (String, usize) {
		let input = session.input_text(ctx).trim_end_matches('\n').to_string();
		let input_len = Self::char_count(&input);
		let cursor = ctx.buffer(session.input).map(|buffer| buffer.cursor).unwrap_or(input_len);
		(input, cursor.min(input_len))
	}

	fn update_completion_state(&mut self, ctx: &mut dyn OverlayContext, items: Vec<CompletionItem>, replace_start: usize, query: String, token_index: usize) {
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

	fn refresh_for(&mut self, ctx: &mut dyn OverlayContext, session: &OverlaySession, input: &str, cursor: usize) {
		let token = Self::token_context(input, cursor);
		let usage = ctx.command_usage_snapshot();
		let items = self.build_items_for_token(&token, ctx, session, &usage);
		self.update_completion_state(ctx, items, token.start, token.query, token.token_index);
	}

	fn move_selection(&mut self, ctx: &mut dyn OverlayContext, delta: isize) -> bool {
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

	fn page_selection(&mut self, ctx: &mut dyn OverlayContext, direction: isize) -> bool {
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

	fn accept_tab_completion(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) -> bool {
		let mut selected = ctx
			.completion_state()
			.and_then(|state| state.selected_idx.and_then(|idx| state.items.get(idx)).or_else(|| state.items.first()))
			.cloned();
		let Some(mut selected_item) = selected.take() else {
			return true;
		};

		let (input, cursor) = Self::current_input_and_cursor(ctx, session);
		let token = Self::token_context(&input, cursor);
		let replace_end = Self::effective_replace_end(&token, cursor);
		let current_replacement: String = input.chars().skip(token.start).take(replace_end.saturating_sub(token.start)).collect();

		if current_replacement == selected_item.insert_text {
			let _ = self.move_selection(ctx, 1);
			if let Some(next) = ctx
				.completion_state()
				.and_then(|state| state.selected_idx.and_then(|idx| state.items.get(idx)).or_else(|| state.items.first()))
				.cloned()
			{
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

	fn effective_replace_end(token: &TokenCtx, cursor: usize) -> usize {
		match (token.quoted, token.close_quote_idx) {
			(Some(_), Some(close_quote_idx)) if cursor > close_quote_idx => close_quote_idx,
			_ => cursor,
		}
	}
}

impl OverlayController for CommandPaletteOverlay {
	fn name(&self) -> &'static str {
		"CommandPalette"
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
			let opt = xeno_registry::db::OPTIONS
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
		match key.code {
			KeyCode::Up => self.move_selection(ctx, -1),
			KeyCode::Down => self.move_selection(ctx, 1),
			KeyCode::PageUp => self.page_selection(ctx, -1),
			KeyCode::PageDown => self.page_selection(ctx, 1),
			KeyCode::Char('n') if key.modifiers.ctrl => self.move_selection(ctx, 1),
			KeyCode::Char('p') if key.modifiers.ctrl => self.move_selection(ctx, -1),
			KeyCode::Tab => self.accept_tab_completion(ctx, session),
			_ => false,
		}
	}

	fn on_commit<'a>(&'a mut self, ctx: &'a mut dyn OverlayContext, session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let input = session.input_text(ctx).trim().to_string();

		if !input.is_empty() {
			let chars: Vec<char> = input.chars().collect();
			let tokens = Self::tokenize(&chars);
			if let Some(name_tok) = tokens.first() {
				let typed_name: String = chars[name_tok.content_start..name_tok.content_end].iter().collect();
				let args: Vec<String> = tokens
					.iter()
					.skip(1)
					.map(|tok| chars[tok.content_start..tok.content_end].iter().collect())
					.collect();
				let token = Self::token_context(&input, Self::char_count(&input));
				let mut command_name = typed_name;

				if token.token_index == 0
					&& let Some(state) = ctx.completion_state()
					&& state.active && let Some(item) = state.selected_idx.and_then(|idx| state.items.get(idx)).or_else(|| state.items.first())
					&& item.kind == CompletionKind::Command
					&& !item.insert_text.is_empty()
				{
					command_name = item.insert_text.clone();
				}

				if let Some(cmd) = crate::commands::find_editor_command(&command_name) {
					ctx.queue_command(cmd.name, args);
					ctx.record_command_usage(cmd.name);
				} else if let Some(cmd) = xeno_registry::commands::find_command(&command_name) {
					let name: &'static str = Box::leak(cmd.name_str().to_string().into_boxed_str());
					ctx.queue_command(name, args);
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

#[cfg(test)]
mod tests;
