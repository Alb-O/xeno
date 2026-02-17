use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, Instant};

use xeno_primitives::{Key, KeyCode, Selection};
use xeno_registry::options::OptionValue;

use crate::completion::{CompletionFileMeta, CompletionItem, CompletionKind, CompletionState, SelectionIntent};
use crate::overlay::picker_engine::model::{CommitDecision, PickerAction};
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy, StatusKind};
use crate::window::GutterSelector;

const FILE_PICKER_LIMIT: usize = 200;
const QUERY_REFRESH_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerQueryMode {
	Indexed,
	Path,
}

pub struct FilePickerOverlay {
	root: Option<PathBuf>,
	root_override: Option<PathBuf>,
	last_input: String,
	selected_label: Option<String>,
	last_indexed_files: usize,
	last_query_sent: Option<Instant>,
}

impl Default for FilePickerOverlay {
	fn default() -> Self {
		Self::new(None)
	}
}

impl FilePickerOverlay {
	pub fn new(root_override: Option<PathBuf>) -> Self {
		Self {
			root: None,
			root_override,
			last_input: String::new(),
			selected_label: None,
			last_indexed_files: 0,
			last_query_sent: None,
		}
	}

	fn resolve_root(&self, ctx: &dyn OverlayContext, session: &OverlaySession) -> PathBuf {
		if let Some(root) = self.root_override.clone() {
			return crate::paths::fast_abs(&root);
		}

		ctx.buffer(session.origin_view)
			.and_then(|buffer| buffer.path())
			.and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
			.map(|path| crate::paths::fast_abs(&path))
			.unwrap_or_else(|| {
				std::env::current_dir()
					.map(|path| crate::paths::fast_abs(&path))
					.unwrap_or_else(|_| PathBuf::from("."))
			})
	}

	fn status_from_progress(&self, ctx: &dyn OverlayContext, session: &mut OverlaySession) {
		let progress = ctx.filesystem().progress();
		if progress.complete {
			session.clear_status();
		} else {
			session.set_status(StatusKind::Info, format!("Indexing... {}", progress.indexed_files));
		}
	}

	fn build_indexed_items(&self, ctx: &dyn OverlayContext, query: &str) -> Vec<CompletionItem> {
		if query.is_empty() {
			return ctx
				.filesystem()
				.data()
				.files
				.iter()
				.take(FILE_PICKER_LIMIT)
				.map(|row| {
					let path_text = row.path.to_string();
					CompletionItem {
						label: path_text.clone(),
						insert_text: path_text.clone(),
						detail: Some("file".into()),
						filter_text: None,
						kind: CompletionKind::File,
						match_indices: None,
						right: Some("file".into()),
						file: Some(CompletionFileMeta::new(path_text, xeno_file_display::FileKind::File)),
					}
				})
				.collect();
		}

		if ctx.filesystem().result_query() != query {
			return Vec::new();
		}

		ctx.filesystem()
			.results()
			.iter()
			.take(FILE_PICKER_LIMIT)
			.map(|row| {
				let path_text = row.path.to_string();
				CompletionItem {
					label: path_text.clone(),
					insert_text: path_text.clone(),
					detail: Some("file".into()),
					filter_text: None,
					kind: CompletionKind::File,
					match_indices: row.match_indices.clone(),
					right: Some("file".into()),
					file: Some(CompletionFileMeta::new(path_text, xeno_file_display::FileKind::File)),
				}
			})
			.collect()
	}

	fn query_mode(query: &str) -> PickerQueryMode {
		if query.is_empty() {
			return PickerQueryMode::Indexed;
		}

		if Self::is_path_like_query(query) {
			PickerQueryMode::Path
		} else {
			PickerQueryMode::Indexed
		}
	}

	fn is_path_like_query(query: &str) -> bool {
		if query.starts_with('/') || query.starts_with('\\') {
			return true;
		}
		if query.starts_with("./") || query.starts_with(".\\") || query.starts_with("../") || query.starts_with("..\\") {
			return true;
		}
		if query == "." || query == ".." {
			return true;
		}
		if query.starts_with('~') {
			return true;
		}
		if query.contains('/') || query.contains('\\') {
			return true;
		}

		let mut chars = query.chars();
		matches!(
			(chars.next(), chars.next()),
			(Some(drive), Some(':')) if drive.is_ascii_alphabetic()
		)
	}

	fn split_path_query(query: &str) -> (String, String) {
		if query == "~" {
			return ("~/".to_string(), String::new());
		}

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

	fn expand_home_path(input: &str) -> PathBuf {
		if input == "~" {
			return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
		}

		if let Some(rest) = input.strip_prefix("~/").or_else(|| input.strip_prefix("~\\")) {
			if let Some(home) = dirs::home_dir() {
				return home.join(rest);
			}
		}

		PathBuf::from(input)
	}

	fn picker_base_dir(&self) -> PathBuf {
		self.root
			.clone()
			.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
	}

	fn resolve_user_path(&self, input: &str) -> PathBuf {
		let base_dir = self.picker_base_dir();
		let path = Self::expand_home_path(input);
		let abs_path = if path.is_absolute() { path } else { base_dir.join(path) };
		crate::paths::normalize_lexical(&abs_path)
	}

	fn resolve_query_directory(&self, dir_part: &str) -> PathBuf {
		if dir_part.is_empty() {
			return self.picker_base_dir();
		}

		let dir = Self::expand_home_path(dir_part);
		if dir.is_absolute() {
			crate::paths::normalize_lexical(&dir)
		} else {
			crate::paths::normalize_lexical(&self.picker_base_dir().join(dir))
		}
	}

	fn build_path_items(&self, query: &str) -> Vec<CompletionItem> {
		let (dir_part, file_part) = Self::split_path_query(query);
		let dir_path = self.resolve_query_directory(&dir_part);
		let Ok(entries) = fs::read_dir(&dir_path) else {
			return Vec::new();
		};

		let mut scored = Vec::new();
		let show_hidden = file_part.starts_with('.');
		let prefix_len = dir_part.chars().count();

		for entry in entries.flatten().take(FILE_PICKER_LIMIT) {
			let name = entry.file_name().to_string_lossy().to_string();
			if !show_hidden && name.starts_with('.') {
				continue;
			}

			let is_dir = entry.file_type().ok().is_some_and(|ft| ft.is_dir());
			let suffix = if is_dir { "/" } else { "" };
			let insert_text = format!("{dir_part}{name}{suffix}");
			let file_kind = if is_dir {
				xeno_file_display::FileKind::Directory
			} else {
				xeno_file_display::FileKind::File
			};
			let file_meta = CompletionFileMeta::new(dir_path.join(&name), file_kind);

			let (score, match_indices) = if file_part.is_empty() {
				(0i32, None)
			} else {
				let Some((score, _, indices)) = crate::completion::frizbee_match(&file_part, &name) else {
					continue;
				};
				let adjusted = if indices.is_empty() {
					None
				} else {
					Some(indices.into_iter().map(|idx| idx.saturating_add(prefix_len)).collect())
				};
				(score as i32, adjusted)
			};

			scored.push((
				score + if is_dir { 40 } else { 0 },
				CompletionItem {
					label: insert_text.clone(),
					insert_text,
					detail: Some(if is_dir { "directory".into() } else { "file".into() }),
					filter_text: None,
					kind: CompletionKind::File,
					match_indices,
					right: Some(if is_dir { "dir".into() } else { "file".into() }),
					file: Some(file_meta),
				},
			));
		}

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));
		scored.into_iter().map(|(_, item)| item).collect()
	}

	fn build_items(&self, ctx: &dyn OverlayContext, query: &str, mode: PickerQueryMode) -> Vec<CompletionItem> {
		match mode {
			PickerQueryMode::Indexed => self.build_indexed_items(ctx, query),
			PickerQueryMode::Path => self.build_path_items(query),
		}
	}

	fn is_directory_item(item: &CompletionItem) -> bool {
		item.file.as_ref().is_some_and(|meta| meta.kind() == xeno_file_display::FileKind::Directory)
	}

	fn update_completion_state(&mut self, ctx: &mut dyn OverlayContext, query: &str, mode: PickerQueryMode) {
		let waiting_for_query = mode == PickerQueryMode::Indexed && !query.is_empty() && ctx.filesystem().result_query() != query;
		if waiting_for_query {
			let state = ctx.completion_state_mut();
			state.show_kind = false;
			state.suppressed = false;
			state.replace_start = 0;
			state.query = query.to_string();
			state.active = !state.items.is_empty();

			if state.items.is_empty() {
				state.selected_idx = None;
				state.selection_intent = SelectionIntent::Auto;
				self.selected_label = None;
			}
			return;
		}

		let items = self.build_items(ctx, query, mode);

		let previous_label = self.selected_label.clone();
		let state = ctx.completion_state_mut();
		state.show_kind = false;
		state.suppressed = false;
		state.replace_start = 0;
		state.query = query.to_string();
		state.scroll_offset = 0;
		state.items = items;
		state.active = !state.items.is_empty();

		if state.items.is_empty() {
			state.selected_idx = None;
			state.selection_intent = SelectionIntent::Auto;
			self.selected_label = None;
			return;
		}

		if let Some(label) = previous_label
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
	}

	fn maybe_issue_query(&mut self, ctx: &mut dyn OverlayContext, query: &str, query_changed: bool, mode: PickerQueryMode) {
		if mode != PickerQueryMode::Indexed {
			return;
		}

		if query.is_empty() {
			return;
		}

		let now = Instant::now();
		let indexed_files = ctx.filesystem().progress().indexed_files;
		let indexing_progressed = indexed_files != self.last_indexed_files;
		let throttle_ready = self
			.last_query_sent
			.map(|at| now.saturating_duration_since(at) >= QUERY_REFRESH_INTERVAL)
			.unwrap_or(true);

		if query_changed || (indexing_progressed && throttle_ready) {
			let _ = ctx.filesystem_mut().query(query.to_string(), FILE_PICKER_LIMIT);
			self.last_query_sent = Some(now);
		}
	}

	fn set_input_text(&mut self, ctx: &mut dyn OverlayContext, session: &OverlaySession, input: &str) {
		ctx.reset_buffer_content(session.input, input);
		if let Some(buffer) = ctx.buffer_mut(session.input) {
			let cursor = input.chars().count();
			buffer.set_cursor_and_selection(cursor, Selection::point(cursor));
		}
	}

	fn selected_item(ctx: &dyn OverlayContext) -> Option<CompletionItem> {
		crate::overlay::picker_engine::decision::selected_completion_item(ctx.completion_state())
	}

	fn handle_enter(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) -> bool {
		let Some(selected) = Self::selected_item(ctx) else {
			return false;
		};
		if !Self::is_directory_item(&selected) {
			return false;
		}

		let mut next_input = selected.insert_text.clone();
		if !next_input.ends_with('/') && !next_input.ends_with('\\') {
			next_input.push('/');
		}

		self.set_input_text(ctx, session, &next_input);
		self.refresh_items(ctx, session, &next_input);
		true
	}

	fn accept_tab_completion(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) -> bool {
		let current_input = session.input_text(ctx).trim_end_matches('\n').to_string();
		let Some(mut selected) = Self::selected_item(ctx) else {
			return false;
		};
		if crate::overlay::picker_engine::decision::is_exact_selection_match(&current_input, &selected) {
			let _ = self.move_selection(ctx, 1);
			let Some(next) = Self::selected_item(ctx) else {
				return true;
			};
			selected = next;
		}

		let mut next_input = selected.insert_text.clone();
		if Self::is_directory_item(&selected) && !next_input.ends_with('/') && !next_input.ends_with('\\') {
			next_input.push('/');
		}

		self.set_input_text(ctx, session, &next_input);
		self.refresh_items(ctx, session, &next_input);
		true
	}

	fn enter_commit_decision(&self, ctx: &dyn OverlayContext) -> CommitDecision {
		let Some(selected) = Self::selected_item(ctx) else {
			return CommitDecision::CommitTyped;
		};

		if Self::is_directory_item(&selected) {
			CommitDecision::ApplySelectionThenStay
		} else {
			CommitDecision::CommitTyped
		}
	}

	fn picker_action_for_key(key: Key) -> Option<PickerAction> {
		match key.code {
			KeyCode::Enter => Some(PickerAction::Commit(CommitDecision::CommitTyped)),
			KeyCode::Tab => Some(PickerAction::ApplySelection),
			KeyCode::Up => Some(PickerAction::MoveSelection { delta: -1 }),
			KeyCode::Down => Some(PickerAction::MoveSelection { delta: 1 }),
			KeyCode::PageUp => Some(PickerAction::PageSelection { direction: -1 }),
			KeyCode::PageDown => Some(PickerAction::PageSelection { direction: 1 }),
			KeyCode::Char('n') if key.modifiers.ctrl => Some(PickerAction::MoveSelection { delta: 1 }),
			KeyCode::Char('p') if key.modifiers.ctrl => Some(PickerAction::MoveSelection { delta: -1 }),
			_ => None,
		}
	}

	fn handle_picker_action(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, action: PickerAction) -> bool {
		match action {
			PickerAction::MoveSelection { delta } => self.move_selection(ctx, delta),
			PickerAction::PageSelection { direction } => self.page_selection(ctx, direction),
			PickerAction::ApplySelection => {
				let _ = self.accept_tab_completion(ctx, session);
				true
			}
			PickerAction::Commit(_) => match self.enter_commit_decision(ctx) {
				CommitDecision::CommitTyped => false,
				CommitDecision::ApplySelectionThenStay => self.handle_enter(ctx, session),
			},
		}
	}

	fn refresh_items(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, text: &str) {
		let query = text.trim_end_matches('\n').to_string();
		let mode = Self::query_mode(&query);
		let query_changed = query != self.last_input;
		self.maybe_issue_query(ctx, &query, query_changed, mode);
		self.update_completion_state(ctx, &query, mode);
		self.status_from_progress(ctx, session);
		self.last_indexed_files = ctx.filesystem().progress().indexed_files;
		self.last_input = query;
		ctx.request_redraw();
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
}

impl OverlayController for FilePickerOverlay {
	fn name(&self) -> &'static str {
		"FilePicker"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		OverlayUiSpec {
			title: Some("File Picker".into()),
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
				.get_key(&xeno_registry::options::keys::CURSORLINE.untyped())
				.expect("cursorline option missing from registry");
			buffer.local_options.set(opt, OptionValue::Bool(false));
		}

		let root = self.resolve_root(ctx, session);
		let options = crate::filesystem::FilesystemOptions {
			threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
			..crate::filesystem::FilesystemOptions::default()
		};
		ctx.filesystem_mut().ensure_index(root.clone(), options);
		self.root = Some(root);

		let text = session.input_text(ctx);
		self.refresh_items(ctx, session, &text);
	}

	fn on_input_changed(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, text: &str) {
		self.refresh_items(ctx, session, text);
	}

	fn on_key(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, key: Key) -> bool {
		let Some(action) = Self::picker_action_for_key(key) else {
			return false;
		};
		self.handle_picker_action(ctx, session, action)
	}

	fn on_commit<'a>(&'a mut self, ctx: &'a mut dyn OverlayContext, session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let selected = Self::selected_item(ctx);
		if let Some(selected) = selected {
			if Self::is_directory_item(&selected) {
				return Box::pin(async {});
			}
			let abs_path = self.resolve_user_path(&selected.insert_text);
			ctx.queue_command("edit", vec![abs_path.to_string_lossy().to_string()]);
			return Box::pin(async {});
		}

		let typed = session.input_text(ctx).trim_end_matches('\n').trim().to_string();
		if !typed.is_empty() {
			let abs_path = self.resolve_user_path(&typed);
			ctx.queue_command("edit", vec![abs_path.to_string_lossy().to_string()]);
		}

		Box::pin(async {})
	}

	fn on_close(&mut self, ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _reason: CloseReason) {
		ctx.clear_completion_state();
		self.last_input.clear();
		self.selected_label = None;
		self.last_indexed_files = 0;
		self.last_query_sent = None;
		self.root = None;
		ctx.request_redraw();
	}
}

#[cfg(test)]
mod tests;
