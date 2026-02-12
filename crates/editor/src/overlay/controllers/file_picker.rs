use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, Instant};

use xeno_primitives::{Key, KeyCode};
use xeno_registry::options::OptionValue;

use crate::completion::{CompletionItem, CompletionKind, CompletionState, SelectionIntent};
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy, StatusKind};
use crate::window::GutterSelector;

const FILE_PICKER_LIMIT: usize = 200;
const QUERY_REFRESH_INTERVAL: Duration = Duration::from_millis(120);

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

	fn build_items(&self, ctx: &dyn OverlayContext, query: &str) -> Vec<CompletionItem> {
		if query.is_empty() {
			return ctx
				.filesystem()
				.data()
				.files
				.iter()
				.take(FILE_PICKER_LIMIT)
				.map(|row| CompletionItem {
					label: row.path.to_string(),
					insert_text: row.path.to_string(),
					detail: Some("file".into()),
					filter_text: None,
					kind: CompletionKind::File,
					match_indices: None,
					right: Some("file".into()),
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
			.map(|row| CompletionItem {
				label: row.path.to_string(),
				insert_text: row.path.to_string(),
				detail: Some("file".into()),
				filter_text: None,
				kind: CompletionKind::File,
				match_indices: row.match_indices.clone(),
				right: Some("file".into()),
			})
			.collect()
	}

	fn update_completion_state(&mut self, ctx: &mut dyn OverlayContext, query: &str) {
		let waiting_for_query = !query.is_empty() && ctx.filesystem().result_query() != query;
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

		let items = self.build_items(ctx, query);

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

	fn maybe_issue_query(&mut self, ctx: &mut dyn OverlayContext, query: &str, query_changed: bool) {
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

	fn refresh_items(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, text: &str) {
		let query = text.trim_end_matches('\n').to_string();
		let query_changed = query != self.last_input;
		self.maybe_issue_query(ctx, &query, query_changed);
		self.update_completion_state(ctx, &query);
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
		let mut options = crate::filesystem::FilesystemOptions::default();
		options.threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
		ctx.filesystem_mut().ensure_index(root.clone(), options);
		self.root = Some(root);

		let text = session.input_text(ctx);
		self.refresh_items(ctx, session, &text);
	}

	fn on_input_changed(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, text: &str) {
		self.refresh_items(ctx, session, text);
	}

	fn on_key(&mut self, ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, key: Key) -> bool {
		match key.code {
			KeyCode::Up => self.move_selection(ctx, -1),
			KeyCode::Down => self.move_selection(ctx, 1),
			KeyCode::PageUp => self.page_selection(ctx, -1),
			KeyCode::PageDown => self.page_selection(ctx, 1),
			KeyCode::Char('n') if key.modifiers.ctrl => self.move_selection(ctx, 1),
			KeyCode::Char('p') if key.modifiers.ctrl => self.move_selection(ctx, -1),
			_ => false,
		}
	}

	fn on_commit<'a>(&'a mut self, ctx: &'a mut dyn OverlayContext, _session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let selected = ctx
			.completion_state()
			.and_then(|state| state.selected_idx.and_then(|idx| state.items.get(idx)).or_else(|| state.items.first()))
			.cloned();

		if let Some(selected) = selected {
			let root = self.root.clone().unwrap_or_else(|| PathBuf::from("."));
			let abs_path = crate::paths::fast_abs(&root).join(&selected.insert_text);
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
