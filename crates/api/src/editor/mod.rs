mod actions;
mod actions_exec;
mod buffer_manager;
pub mod extensions;
mod history;
mod hook_runtime;
mod input_handling;
mod layout_manager;
mod messaging;
mod navigation;
mod search;
mod separator;
pub mod types;
mod views;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentfs_sdk::filesystem::FileSystem;
use agentfs_sdk::filesystem::hostfs::HostFS;
pub use buffer_manager::BufferManager;
use evildoer_base::Transaction;
use evildoer_language::LanguageLoader;
use evildoer_manifest::syntax::SyntaxStyles;
use evildoer_manifest::{HookContext, HookEventData, Mode, Theme, emit_hook, emit_hook_sync_with};
pub use hook_runtime::HookRuntime;
pub use layout_manager::LayoutManager;
pub use types::{HistoryEntry, Registers};

pub use self::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
use crate::buffer::{BufferId, BufferView, TerminalId};
use crate::editor::extensions::{EXTENSIONS, ExtensionMap, StyleOverlays};
use crate::editor::types::CompletionState;
use crate::ui::UiManager;

/// The main editor/workspace structure.
///
/// Contains text buffers, terminals, and manages workspace-level state including
/// theme, UI panels, notifications, and extensions. Supports split views with
/// heterogeneous content (text buffers and terminals in the same layout).
///
/// # View System
///
/// The editor tracks focus via [`BufferView`], which can be either a text buffer
/// or a terminal. The layout tree arranges views in splits:
///
/// ```text
/// ┌────────────────┬────────────────┐
/// │  Text Buffer   │   Terminal     │
/// │   (focused)    │                │
/// └────────────────┴────────────────┘
/// ```
///
/// # Creating an Editor
///
/// ```ignore
/// // Open a file
/// let editor = Editor::new(PathBuf::from("src/main.rs")).await?;
///
/// // Create a scratch buffer
/// let editor = Editor::new_scratch();
/// ```
///
/// # Focus and Navigation
///
/// - [`focused_view`] - Current focus (text or terminal)
/// - [`focus_buffer`] / [`focus_terminal`] - Focus by ID
/// - [`focus_next_view`] / [`focus_prev_view`] - Cycle through views
///
/// [`BufferView`]: crate::buffer::BufferView
/// [`focused_view`]: Self::focused_view
/// [`focus_buffer`]: Self::focus_buffer
/// [`focus_terminal`]: Self::focus_terminal
/// [`focus_next_view`]: Self::focus_next_view
/// [`focus_prev_view`]: Self::focus_prev_view
pub struct Editor {
	/// Buffer and terminal management.
	pub buffers: BufferManager,

	/// Layout and split management.
	pub layout: LayoutManager,

	/// Workspace-level registers (yank buffer, etc.).
	pub registers: Registers,

	/// Current theme.
	pub theme: &'static Theme,

	/// Window dimensions.
	pub window_width: Option<u16>,
	pub window_height: Option<u16>,

	/// UI manager (panels, dock, etc.).
	pub ui: UiManager,

	/// Whether a redraw is needed.
	pub needs_redraw: bool,

	/// Whether a command requested the editor to quit.
	pending_quit: bool,

	/// Notification system.
	pub notifications: evildoer_tui::widgets::notifications::ToastManager,

	/// Last tick timestamp.
	pub last_tick: std::time::SystemTime,

	/// Completion state.
	pub completions: CompletionState,

	/// Extension map (typemap for extension state).
	pub extensions: ExtensionMap,

	/// Filesystem abstraction.
	pub fs: Arc<dyn FileSystem>,

	/// Language configuration loader.
	pub language_loader: LanguageLoader,

	/// Style overlays for rendering modifications.
	pub style_overlays: StyleOverlays,

	/// Runtime for scheduling async hooks during sync emission.
	pub hook_runtime: HookRuntime,

	/// Buffers with pending content changes for [`HookEvent::BufferChange`].
	dirty_buffers: HashSet<BufferId>,

	/// The docked terminal (opened via `:` key, only one allowed).
	docked_terminal: Option<TerminalId>,

	/// Views with sticky focus (resist mouse hover focus changes).
	sticky_views: HashSet<BufferView>,
}

impl evildoer_manifest::editor_ctx::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		// Terminals are never considered "modified" for save purposes
		if self.is_terminal_focused() {
			return false;
		}
		self.buffer().modified()
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), evildoer_manifest::CommandError>> + '_>,
	> {
		Box::pin(async move {
			// Cannot save a terminal
			if self.is_terminal_focused() {
				return Err(evildoer_manifest::CommandError::InvalidArgument(
					"Cannot save a terminal".to_string(),
				));
			}

			let path_owned = match &self.buffer().path() {
				Some(p) => p.clone(),
				None => {
					return Err(evildoer_manifest::CommandError::InvalidArgument(
						"No filename. Use :write <filename>".to_string(),
					));
				}
			};

			let text_slice = self.buffer().doc().content.clone();
			emit_hook(&HookContext::new(
				HookEventData::BufferWritePre {
					path: &path_owned,
					text: text_slice.slice(..),
				},
				Some(&self.extensions),
			))
			.await;

			let mut content = Vec::new();
			for chunk in self.buffer().doc().content.chunks() {
				content.extend_from_slice(chunk.as_bytes());
			}

			let virtual_path = self.path_to_virtual(&path_owned).ok_or_else(|| {
				evildoer_manifest::CommandError::Io(format!(
					"Path contains invalid UTF-8: {}",
					path_owned.display()
				))
			})?;
			self.fs
				.write_file(&virtual_path, &content)
				.await
				.map_err(|e| evildoer_manifest::CommandError::Io(e.to_string()))?;

			self.buffer_mut().set_modified(false);
			self.notify("info", format!("Saved {}", path_owned.display()));

			emit_hook(&HookContext::new(
				HookEventData::BufferWrite { path: &path_owned },
				Some(&self.extensions),
			))
			.await;

			Ok(())
		})
	}

	fn save_as(
		&mut self,
		path: PathBuf,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), evildoer_manifest::CommandError>> + '_>,
	> {
		// Cannot save a terminal
		if self.is_terminal_focused() {
			return Box::pin(async {
				Err(evildoer_manifest::CommandError::InvalidArgument(
					"Cannot save a terminal".to_string(),
				))
			});
		}

		self.buffer_mut().set_path(Some(path));
		self.save()
	}
}

impl evildoer_manifest::EditorOps for Editor {}

impl Editor {
	pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
		let cwd = std::env::current_dir()?;
		let fs = Arc::new(HostFS::new(cwd.clone())?);

		let virtual_path = Self::compute_virtual_path(&path, &cwd)
			.ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {}", path.display()))?;

		let content = if fs.stat(&virtual_path).await?.is_some() {
			let bytes = fs.read_file(&virtual_path).await?.unwrap_or_default();
			String::from_utf8_lossy(&bytes).to_string()
		} else {
			String::new()
		};

		Ok(Self::from_content(fs, content, Some(path)))
	}

	fn compute_virtual_path(path: &Path, cwd: &Path) -> Option<String> {
		let path_str = path.to_str()?;

		if path.is_absolute()
			&& let Ok(relative) = path.strip_prefix(cwd)
		{
			return relative.to_str().map(String::from);
		}

		Some(path_str.to_string())
	}

	pub fn new_scratch() -> Self {
		let fs = Arc::new(HostFS::new(std::env::current_dir().unwrap()).unwrap());
		Self::from_content(fs, String::new(), None)
	}

	pub fn from_content(fs: Arc<dyn FileSystem>, content: String, path: Option<PathBuf>) -> Self {
		// Initialize language loader from embedded languages.kdl
		let language_loader = LanguageLoader::from_embedded();

		// Create buffer manager with initial buffer
		let buffer_manager = BufferManager::new(content, path.clone(), &language_loader);
		let buffer_id = buffer_manager.focused_buffer_id().unwrap();

		let mut hook_runtime = HookRuntime::new();

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = buffer_manager.focused_buffer();

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::BufferOpen {
					path: hook_path,
					text: buffer.doc().content.slice(..),
					file_type: buffer.file_type().as_deref(),
				},
				None,
			),
			&mut hook_runtime,
		);

		Self {
			buffers: buffer_manager,
			layout: LayoutManager::new(buffer_id),
			registers: Registers::default(),
			theme: evildoer_manifest::get_theme(evildoer_manifest::DEFAULT_THEME_ID)
				.unwrap_or(&evildoer_manifest::DEFAULT_THEME),
			window_width: None,
			window_height: None,
			ui: UiManager::new(),
			needs_redraw: false,
			pending_quit: false,
			notifications: evildoer_tui::widgets::notifications::ToastManager::new()
				.max_visible(Some(5))
				.overflow(evildoer_tui::widgets::notifications::Overflow::DropOldest),
			last_tick: std::time::SystemTime::now(),
			completions: CompletionState::default(),
			extensions: {
				let mut map = ExtensionMap::new();
				let mut sorted_exts: Vec<_> = EXTENSIONS.iter().collect();
				sorted_exts.sort_by_key(|e| e.priority);
				for ext in sorted_exts {
					(ext.init)(&mut map);
				}
				map
			},
			fs,
			language_loader,
			style_overlays: StyleOverlays::new(),
			hook_runtime,
			dirty_buffers: HashSet::new(),
			docked_terminal: None,
			sticky_views: HashSet::new(),
		}
	}

	/// Opens a new buffer from content, optionally with a path.
	///
	/// This async version awaits all hooks including async ones (e.g., LSP).
	/// For sync contexts like split operations, use [`open_buffer_sync`](Self::open_buffer_sync).
	pub async fn open_buffer(&mut self, content: String, path: Option<PathBuf>) -> BufferId {
		let buffer_id = self.buffers.create_buffer(
			content,
			path.clone(),
			&self.language_loader,
			self.window_width,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = self.buffers.get_buffer(buffer_id).unwrap();

		let text_slice = buffer.doc().content.clone();
		let file_type = buffer.file_type();
		emit_hook(&HookContext::new(
			HookEventData::BufferOpen {
				path: hook_path,
				text: text_slice.slice(..),
				file_type: file_type.as_deref(),
			},
			Some(&self.extensions),
		))
		.await;

		buffer_id
	}

	/// Opens a new buffer synchronously, scheduling async hooks for later.
	///
	/// Use this in sync contexts like split operations. Async hooks are queued
	/// in the hook runtime and will execute when the main loop drains them.
	pub fn open_buffer_sync(&mut self, content: String, path: Option<PathBuf>) -> BufferId {
		let buffer_id = self.buffers.create_buffer(
			content,
			path.clone(),
			&self.language_loader,
			self.window_width,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = self.buffers.get_buffer(buffer_id).unwrap();

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::BufferOpen {
					path: hook_path,
					text: buffer.doc().content.slice(..),
					file_type: buffer.file_type().as_deref(),
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);

		buffer_id
	}

	/// Opens a file as a new buffer.
	///
	/// Returns the new buffer's ID, or an error if the file couldn't be read.
	pub async fn open_file(&mut self, path: PathBuf) -> anyhow::Result<BufferId> {
		let cwd = std::env::current_dir()?;
		let virtual_path = Self::compute_virtual_path(&path, &cwd)
			.ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {}", path.display()))?;

		let content = if self.fs.stat(&virtual_path).await?.is_some() {
			let bytes = self.fs.read_file(&virtual_path).await?.unwrap_or_default();
			String::from_utf8_lossy(&bytes).to_string()
		} else {
			String::new()
		};

		Ok(self.open_buffer(content, Some(path)).await)
	}

	/// Focuses a specific view explicitly (user action like click or keybinding).
	///
	/// Returns true if the view exists and was focused.
	/// Explicit focus can override sticky focus and will close dockables.
	pub fn focus_view(&mut self, view: BufferView) -> bool {
		self.focus_view_inner(view, true)
	}

	/// Focuses a specific view implicitly (mouse hover).
	///
	/// Returns true if the view exists and was focused.
	/// Respects sticky focus - won't steal focus from sticky views.
	pub fn focus_view_implicit(&mut self, view: BufferView) -> bool {
		let current = self.buffers.focused_view();
		if current == view || self.sticky_views.contains(&current) {
			return false;
		}
		self.focus_view_inner(view, false)
	}

	fn focus_view_inner(&mut self, view: BufferView, explicit: bool) -> bool {
		let old_view = self.buffers.focused_view();
		if !self.buffers.set_focused_view(view) {
			return false;
		}
		self.needs_redraw = true;

		if explicit
			&& let Some(docked_id) = self.docked_terminal
			&& old_view == BufferView::Terminal(docked_id)
			&& view != old_view
		{
			self.sticky_views.remove(&old_view);
			self.hide_view(old_view);
		}

		true
	}

	/// Focuses a specific buffer by ID.
	///
	/// Returns true if the buffer exists and was focused.
	pub fn focus_buffer(&mut self, id: BufferId) -> bool {
		self.focus_view(BufferView::Text(id))
	}

	/// Focuses a specific terminal by ID.
	///
	/// Returns true if the terminal exists and was focused.
	pub fn focus_terminal(&mut self, id: TerminalId) -> bool {
		self.focus_view(BufferView::Terminal(id))
	}

	/// Focuses the next view in the layout (buffer or terminal).
	pub fn focus_next_view(&mut self) {
		let next = self.layout.next_view(self.buffers.focused_view());
		self.focus_view(next);
	}

	/// Focuses the previous view in the layout.
	pub fn focus_prev_view(&mut self) {
		let prev = self.layout.prev_view(self.buffers.focused_view());
		self.focus_view(prev);
	}

	/// Focuses the next text buffer in the layout.
	pub fn focus_next_buffer(&mut self) {
		if let Some(current_id) = self.buffers.focused_view().as_text() {
			let next_id = self.layout.next_buffer(current_id);
			self.focus_buffer(next_id);
		}
	}

	/// Focuses the previous text buffer in the layout.
	pub fn focus_prev_buffer(&mut self) {
		if let Some(current_id) = self.buffers.focused_view().as_text() {
			let prev_id = self.layout.prev_buffer(current_id);
			self.focus_buffer(prev_id);
		}
	}

	/// Creates a horizontal split with the current view and a new buffer below.
	///
	/// Matches Vim's `:split` / Helix's `hsplit` (Ctrl+w s).
	pub fn split_horizontal(&mut self, new_buffer_id: BufferId) {
		let current_view = self.buffers.focused_view();
		self.layout.split_horizontal(current_view, new_buffer_id);
		self.focus_buffer(new_buffer_id);
	}

	/// Creates a vertical split with the current view and a new buffer to the right.
	///
	/// Matches Vim's `:vsplit` / Helix's `vsplit` (Ctrl+w v).
	pub fn split_vertical(&mut self, new_buffer_id: BufferId) {
		let current_view = self.buffers.focused_view();
		self.layout.split_vertical(current_view, new_buffer_id);
		self.focus_buffer(new_buffer_id);
	}

	/// Creates a new buffer that shares the same document as the current buffer.
	///
	/// This is used for split operations - both buffers see the same content
	/// but have independent cursor/selection/scroll state.
	pub fn clone_buffer_for_split(&mut self) -> BufferId {
		self.buffers.clone_focused_buffer_for_split()
	}

	/// Opens a new terminal in a horizontal split (terminal below).
	pub fn split_horizontal_terminal(&mut self) -> TerminalId {
		let terminal_id = self.create_terminal();
		let current_view = self.buffers.focused_view();
		self.layout
			.split_horizontal_terminal(current_view, terminal_id);
		self.focus_terminal(terminal_id);
		terminal_id
	}

	/// Opens a new terminal in a vertical split (terminal to the right).
	pub fn split_vertical_terminal(&mut self) -> TerminalId {
		let terminal_id = self.create_terminal();
		let current_view = self.buffers.focused_view();
		self.layout
			.split_vertical_terminal(current_view, terminal_id);
		self.focus_terminal(terminal_id);
		terminal_id
	}

	/// Creates a new terminal.
	fn create_terminal(&mut self) -> TerminalId {
		self.buffers.create_terminal()
	}

	/// Toggles the docked terminal.
	///
	/// If visible, hides it (preserving state). Otherwise shows it at the bottom.
	pub fn toggle_terminal(&mut self) {
		let terminal_view = self
			.docked_terminal
			.map(|id| BufferView::Terminal(id));

		if let Some(view) = terminal_view
			&& self.layout.contains_view(view)
		{
			self.sticky_views.remove(&view);
			self.hide_view(view);
			return;
		}

		// Reuse existing terminal or create new one
		let terminal_id = self.docked_terminal.unwrap_or_else(|| {
			let id = self.create_terminal();
			self.docked_terminal = Some(id);
			id
		});

		let terminal_view = BufferView::Terminal(terminal_id);
		self.sticky_views.insert(terminal_view);
		let current_view = self.buffers.focused_view();
		self.layout
			.split_horizontal_terminal(current_view, terminal_id);
		self.focus_terminal(terminal_id);
	}

	/// Hides a view from the layout without destroying it.
	fn hide_view(&mut self, view: BufferView) {
		if self.layout.count() <= 1 {
			return;
		}

		let new_focus = self.layout.remove_view(view);

		if self.buffers.focused_view() == view
			&& let Some(focus) = new_focus
		{
			self.buffers.set_focused_view(focus);
		}

		self.needs_redraw = true;
	}

	pub fn request_quit(&mut self) {
		self.pending_quit = true;
	}

	pub fn take_quit_request(&mut self) -> bool {
		if self.pending_quit {
			self.pending_quit = false;
			true
		} else {
			false
		}
	}

	/// Closes a view (buffer or terminal).
	///
	/// Returns true if the view was closed.
	pub fn close_view(&mut self, view: BufferView) -> bool {
		// Don't close the last view
		if self.layout.count() <= 1 {
			return false;
		}

		if let BufferView::Text(id) = view
			&& let Some(buffer) = self.buffers.get_buffer(id)
		{
			let scratch_path = PathBuf::from("[scratch]");
			let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
			let file_type = buffer.file_type();
			emit_hook_sync_with(
				&HookContext::new(
					HookEventData::BufferClose {
						path: &path,
						file_type: file_type.as_deref(),
					},
					Some(&self.extensions),
				),
				&mut self.hook_runtime,
			);
		}

		// Remove from layout - returns the new focus target if successful
		let new_focus = self.layout.remove_view(view);
		if new_focus.is_none() {
			return false;
		}

		// Remove the actual buffer/terminal
		match view {
			BufferView::Text(id) => {
				self.buffers.remove_buffer(id);
			}
			BufferView::Terminal(id) => {
				self.buffers.remove_terminal(id);
			}
		}

		// If we closed the focused view, focus another one
		if self.buffers.focused_view() == view
			&& let Some(focus) = new_focus
		{
			self.buffers.set_focused_view(focus);
		}

		self.needs_redraw = true;
		true
	}

	/// Closes a buffer.
	///
	/// Returns true if the buffer was closed.
	pub fn close_buffer(&mut self, id: BufferId) -> bool {
		self.close_view(BufferView::Text(id))
	}

	/// Closes a terminal.
	///
	/// Returns true if the terminal was closed.
	pub fn close_terminal(&mut self, id: TerminalId) -> bool {
		self.close_view(BufferView::Terminal(id))
	}

	/// Closes the current view (buffer or terminal).
	///
	/// Returns true if the view was closed.
	pub fn close_current_view(&mut self) -> bool {
		self.close_view(self.buffers.focused_view())
	}

	/// Closes the current buffer if a text buffer is focused.
	///
	/// Returns true if the buffer was closed.
	pub fn close_current_buffer(&mut self) -> bool {
		match self.buffers.focused_view() {
			BufferView::Text(id) => self.close_buffer(id),
			BufferView::Terminal(_) => false,
		}
	}

	pub fn mode(&self) -> Mode {
		if self.is_terminal_focused() {
			// Check if we're in window mode (using first buffer's input handler)
			if let Some(first_buffer_id) = self.layout.first_buffer()
				&& let Some(buffer) = self.buffers.get_buffer(first_buffer_id)
			{
				let mode = buffer.input.mode();
				if matches!(mode, Mode::Window) {
					return mode;
				}
			}
			Mode::Insert // Terminal is always in "insert" mode effectively
		} else {
			self.buffer().input.mode()
		}
	}

	pub fn mode_name(&self) -> &'static str {
		if self.is_terminal_focused() {
			// Check if we're in window mode (using first buffer's input handler)
			if let Some(first_buffer_id) = self.layout.first_buffer()
				&& let Some(buffer) = self.buffers.get_buffer(first_buffer_id)
				&& matches!(buffer.input.mode(), Mode::Window)
			{
				return buffer.input.mode_name();
			}
			"TERMINAL"
		} else {
			self.buffer().input.mode_name()
		}
	}

	pub fn ui_startup(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.startup();
		self.ui = ui;
		self.needs_redraw = true;
	}

	pub fn ui_tick(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
	}

	pub fn tick(&mut self) {
		use std::time::Duration;

		use evildoer_manifest::SplitBuffer;

		use crate::editor::extensions::TICK_EXTENSIONS;

		// Tick all terminals
		let terminal_ids: Vec<_> = self.buffers.terminal_ids().collect();
		for id in terminal_ids {
			if let Some(terminal) = self.buffers.get_terminal_mut(id) {
				let result = terminal.tick(Duration::from_millis(16));
				if result.needs_redraw {
					self.needs_redraw = true;
				}
				if result.wants_close {
					// Terminal exited, close it
					self.close_terminal(id);
				}
			}
		}

		let mut sorted_ticks: Vec<_> = TICK_EXTENSIONS.iter().collect();
		sorted_ticks.sort_by_key(|e| e.priority);
		for ext in sorted_ticks {
			(ext.tick)(self);
		}

		// Check if separator animation needs continuous redraws
		if self.layout.animation_needs_redraw() {
			self.needs_redraw = true;
		}

		let dirty_ids: Vec<_> = self.dirty_buffers.drain().collect();
		for buffer_id in dirty_ids {
			if let Some(buffer) = self.buffers.get_buffer(buffer_id) {
				let scratch_path = PathBuf::from("[scratch]");
				let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
				let file_type = buffer.file_type();
				let version = buffer.version();
				let content = buffer.doc().content.clone();
				emit_hook_sync_with(
					&HookContext::new(
						HookEventData::BufferChange {
							path: &path,
							text: content.slice(..),
							file_type: file_type.as_deref(),
							version,
						},
						Some(&self.extensions),
					),
					&mut self.hook_runtime,
				);
			}
		}
		emit_hook_sync_with(
			&HookContext::new(HookEventData::EditorTick, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	pub fn update_style_overlays(&mut self) {
		use crate::editor::extensions::RENDER_EXTENSIONS;

		self.style_overlays.clear();

		let mut sorted: Vec<_> = RENDER_EXTENSIONS.iter().collect();
		sorted.sort_by_key(|e| e.priority);
		for ext in sorted {
			(ext.update)(self);
		}

		if self.style_overlays.has_animations() {
			self.needs_redraw = true;
		}
	}

	pub fn any_panel_open(&self) -> bool {
		self.ui.any_panel_open()
	}

	pub fn insert_text(&mut self, text: &str) {
		self.buffer_mut().insert_text(text);
		if let BufferView::Text(id) = self.buffers.focused_view() {
			self.dirty_buffers.insert(id);
		}
	}

	pub fn yank_selection(&mut self) {
		if let Some((text, count)) = self.buffer().yank_selection() {
			self.registers.yank = text;
			self.notify("info", format!("Yanked {} chars", count));
		}
	}

	pub fn paste_after(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		let yank = self.registers.yank.clone();
		self.buffer_mut().paste_after(&yank);
		if let BufferView::Text(id) = self.buffers.focused_view() {
			self.dirty_buffers.insert(id);
		}
	}

	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		let yank = self.registers.yank.clone();
		self.buffer_mut().paste_before(&yank);
		if let BufferView::Text(id) = self.buffers.focused_view() {
			self.dirty_buffers.insert(id);
		}
	}

	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.window_width = Some(width);
		self.window_height = Some(height);

		// Update text width for all buffers
		for buffer in self.buffers.buffers_mut() {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let mut ui = std::mem::take(&mut self.ui);
		ui.notify_resize(self, width, height);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
		self.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowResize { width, height },
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
	}

	pub fn handle_focus_in(&mut self) {
		self.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusGained, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	pub fn handle_focus_out(&mut self) {
		self.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusLost, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	pub fn handle_paste(&mut self, content: String) {
		let mut ui = std::mem::take(&mut self.ui);
		let handled = ui.handle_paste(self, content.clone());
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		if handled {
			self.needs_redraw = true;
			return;
		}

		self.insert_text(&content);
	}

	pub fn delete_selection(&mut self) {
		if self.buffer_mut().delete_selection()
			&& let BufferView::Text(id) = self.buffers.focused_view()
		{
			self.dirty_buffers.insert(id);
		}
	}

	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), evildoer_manifest::CommandError> {
		if let Some(theme) = evildoer_manifest::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = evildoer_manifest::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(evildoer_manifest::CommandError::Failed(err))
		}
	}

	pub fn set_filesystem(&mut self, fs: Arc<dyn FileSystem>) {
		self.fs = fs;
	}

	pub fn collect_highlight_spans(
		&self,
		area: evildoer_tui::layout::Rect,
	) -> Vec<(
		evildoer_language::highlight::HighlightSpan,
		evildoer_tui::style::Style,
	)> {
		let buffer = self.buffer();
		let doc = buffer.doc();

		let Some(ref syntax) = doc.syntax else {
			return Vec::new();
		};

		let start_line = buffer.scroll_line;
		let end_line = (start_line + area.height as usize).min(doc.content.len_lines());

		let start_byte = doc.content.line_to_byte(start_line) as u32;
		let end_byte = if end_line < doc.content.len_lines() {
			doc.content.line_to_byte(end_line) as u32
		} else {
			doc.content.len_bytes() as u32
		};

		let highlight_styles = evildoer_language::highlight::HighlightStyles::new(
			SyntaxStyles::scope_names(),
			|scope| self.theme.colors.syntax.resolve(scope),
		);

		let highlighter = syntax.highlighter(
			doc.content.slice(..),
			&self.language_loader,
			start_byte..end_byte,
		);

		highlighter
			.map(|span| {
				let abstract_style = highlight_styles.style_for_highlight(span.highlight);
				let evildoer_tui_style: evildoer_tui::style::Style = abstract_style.into();
				(span, evildoer_tui_style)
			})
			.collect()
	}

	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(
			evildoer_language::highlight::HighlightSpan,
			evildoer_tui::style::Style,
		)],
	) -> Option<evildoer_tui::style::Style> {
		for (span, style) in spans.iter().rev() {
			if byte_pos >= span.start as usize && byte_pos < span.end as usize {
				return Some(*style);
			}
		}
		None
	}

	pub fn apply_style_overlay(
		&self,
		byte_pos: usize,
		style: Option<evildoer_tui::style::Style>,
	) -> Option<evildoer_tui::style::Style> {
		use evildoer_tui::animation::Animatable;

		use crate::editor::extensions::StyleMod;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to evildoer_tui color for blending
				let bg: evildoer_tui::style::Color = self.theme.colors.ui.bg.into();
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(evildoer_tui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => style.fg(color),
			StyleMod::Bg(color) => style.bg(color),
		};

		Some(modified)
	}

	pub fn apply_transaction(&mut self, tx: &Transaction) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist");
		buffer.apply_transaction_with_syntax(tx, &self.language_loader);
		self.dirty_buffers.insert(buffer_id);
	}

	pub fn reparse_syntax(&mut self) {
		let BufferView::Text(buffer_id) = self.buffers.focused_view() else {
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.language_loader);
	}

	fn path_to_virtual(&self, path: &Path) -> Option<String> {
		let cwd = std::env::current_dir().ok()?;
		Self::compute_virtual_path(path, &cwd)
	}
}
