mod actions;
mod actions_exec;
pub mod extensions;
mod history;
mod input_handling;
mod messaging;
mod navigation;
mod search;
mod separator;
pub mod types;
mod views;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentfs_sdk::{FileSystem, HostFS};
use tome_base::Transaction;
use tome_language::LanguageLoader;
use tome_manifest::syntax::SyntaxStyles;
use tome_manifest::{HookContext, Mode, emit_hook, emit_hook_sync};
use tome_theme::Theme;
pub use types::{HistoryEntry, Registers};

pub use self::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
use crate::buffer::{Buffer, BufferId, BufferView, Layout, SplitDirection, TerminalId};
use crate::editor::extensions::{EXTENSIONS, ExtensionMap, StyleOverlays};
use crate::editor::types::CompletionState;
use crate::terminal::TerminalBuffer;
use crate::terminal_ipc::TerminalIpc;
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
	/// All open text buffers, keyed by BufferId.
	buffers: HashMap<BufferId, Buffer>,

	/// All open terminal buffers, keyed by TerminalId.
	terminals: HashMap<TerminalId, TerminalBuffer>,

	/// Counter for generating unique buffer IDs.
	next_buffer_id: u64,

	/// Counter for generating unique terminal IDs.
	next_terminal_id: u64,

	/// The currently focused view (text buffer or terminal).
	focused_view: BufferView,

	/// Layout of buffer views (for splits).
	pub layout: Layout,
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
	pub notifications: tome_tui::widgets::notifications::ToastManager,

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

	/// Currently hovered separator (for visual feedback during resize).
	///
	/// Contains the separator's direction and screen rectangle when the mouse
	/// is hovering over a split boundary. Only set when velocity is low enough.
	pub hovered_separator: Option<(SplitDirection, tome_tui::layout::Rect)>,

	/// Separator the mouse is currently over (regardless of velocity).
	///
	/// This tracks the physical position even when hover is suppressed due to
	/// fast mouse movement, allowing us to activate hover when mouse slows down.
	pub separator_under_mouse: Option<(SplitDirection, tome_tui::layout::Rect)>,

	/// Animation state for separator hover fade effects.
	///
	/// Tracks ongoing hover animations for smooth visual transitions.
	pub separator_hover_animation: Option<SeparatorHoverAnimation>,

	/// Tracks mouse velocity to suppress hover effects during fast movement.
	pub mouse_velocity: MouseVelocityTracker,

	/// Active separator drag state for resizing splits.
	///
	/// When dragging a separator, this contains the separator's direction,
	/// its current rectangle, and the parent split's area (needed to calculate
	/// the new ratio based on mouse position).
	pub dragging_separator: Option<DragState>,

	/// IPC infrastructure for terminal command integration.
	///
	/// Lazily initialized when the first terminal is opened.
	terminal_ipc: Option<TerminalIpc>,
}

impl tome_manifest::editor_ctx::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		// Terminals are never considered "modified" for save purposes
		if self.is_terminal_focused() {
			return false;
		}
		self.buffer().modified
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), tome_manifest::CommandError>> + '_>,
	> {
		Box::pin(async move {
			// Cannot save a terminal
			if self.is_terminal_focused() {
				return Err(tome_manifest::CommandError::InvalidArgument(
					"Cannot save a terminal".to_string(),
				));
			}

			let path_owned = match &self.buffer().path {
				Some(p) => p.clone(),
				None => {
					return Err(tome_manifest::CommandError::InvalidArgument(
						"No filename. Use :write <filename>".to_string(),
					));
				}
			};

			emit_hook(&HookContext::BufferWritePre {
				path: &path_owned,
				text: self.buffer().doc.slice(..),
			})
			.await;

			let mut content = Vec::new();
			for chunk in self.buffer().doc.chunks() {
				content.extend_from_slice(chunk.as_bytes());
			}

			let virtual_path = self.path_to_virtual(&path_owned).ok_or_else(|| {
				tome_manifest::CommandError::Io(format!(
					"Path contains invalid UTF-8: {}",
					path_owned.display()
				))
			})?;
			self.fs
				.write_file(&virtual_path, &content)
				.await
				.map_err(|e| tome_manifest::CommandError::Io(e.to_string()))?;

			self.buffer_mut().modified = false;
			self.notify("info", format!("Saved {}", path_owned.display()));

			emit_hook(&HookContext::BufferWrite { path: &path_owned }).await;

			Ok(())
		})
	}

	fn save_as(
		&mut self,
		path: PathBuf,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), tome_manifest::CommandError>> + '_>,
	> {
		// Cannot save a terminal
		if self.is_terminal_focused() {
			return Box::pin(async {
				Err(tome_manifest::CommandError::InvalidArgument(
					"Cannot save a terminal".to_string(),
				))
			});
		}

		self.buffer_mut().path = Some(path);
		self.save()
	}
}

impl tome_manifest::EditorOps for Editor {}

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
		// Initialize language loader
		let mut language_loader = LanguageLoader::new();
		for lang in tome_manifest::LANGUAGES.iter() {
			language_loader.register(lang.into());
		}

		// Create initial buffer with ID 1
		let buffer_id = BufferId(1);
		let mut buffer = Buffer::new(buffer_id, content.clone(), path.clone());
		buffer.init_syntax(&language_loader);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);

		emit_hook_sync(&HookContext::BufferOpen {
			path: hook_path,
			text: buffer.doc.slice(..),
			file_type: buffer.file_type.as_deref(),
		});

		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		Self {
			buffers,
			terminals: HashMap::new(),
			next_buffer_id: 2, // Next ID will be 2
			next_terminal_id: 1,
			focused_view: BufferView::Text(buffer_id),
			layout: Layout::text(buffer_id),
			registers: Registers::default(),
			theme: tome_theme::get_theme(tome_theme::DEFAULT_THEME_ID)
				.unwrap_or(&tome_theme::DEFAULT_THEME),
			window_width: None,
			window_height: None,
			ui: UiManager::new(),
			needs_redraw: false,
			pending_quit: false,
			notifications: tome_tui::widgets::notifications::ToastManager::new()
				.max_visible(Some(5))
				.overflow(tome_tui::widgets::notifications::Overflow::DropOldest),
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
			hovered_separator: None,
			separator_under_mouse: None,
			separator_hover_animation: None,
			mouse_velocity: MouseVelocityTracker::default(),
			dragging_separator: None,
			terminal_ipc: None,
		}
	}

	/// Opens a new buffer from content, optionally with a path.
	///
	/// Returns the new buffer's ID.
	pub fn open_buffer(&mut self, content: String, path: Option<PathBuf>) -> BufferId {
		let buffer_id = BufferId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let mut buffer = Buffer::new(buffer_id, content.clone(), path.clone());
		buffer.init_syntax(&self.language_loader);

		// Update text width to match current window
		if let Some(width) = self.window_width {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);

		emit_hook_sync(&HookContext::BufferOpen {
			path: hook_path,
			text: buffer.doc.slice(..),
			file_type: buffer.file_type.as_deref(),
		});

		self.buffers.insert(buffer_id, buffer);
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

		Ok(self.open_buffer(content, Some(path)))
	}

	/// Focuses a specific view.
	///
	/// Returns true if the view exists and was focused.
	pub fn focus_view(&mut self, view: BufferView) -> bool {
		let exists = match view {
			BufferView::Text(id) => self.buffers.contains_key(&id),
			BufferView::Terminal(id) => self.terminals.contains_key(&id),
		};
		if exists {
			self.focused_view = view;
			self.needs_redraw = true;
			true
		} else {
			false
		}
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
		let next = self.layout.next_view(self.focused_view);
		self.focus_view(next);
	}

	/// Focuses the previous view in the layout.
	pub fn focus_prev_view(&mut self) {
		let prev = self.layout.prev_view(self.focused_view);
		self.focus_view(prev);
	}

	/// Focuses the next text buffer in the layout.
	pub fn focus_next_buffer(&mut self) {
		if let Some(current_id) = self.focused_view.as_text() {
			let next_id = self.layout.next_buffer(current_id);
			self.focus_buffer(next_id);
		}
	}

	/// Focuses the previous text buffer in the layout.
	pub fn focus_prev_buffer(&mut self) {
		if let Some(current_id) = self.focused_view.as_text() {
			let prev_id = self.layout.prev_buffer(current_id);
			self.focus_buffer(prev_id);
		}
	}

	/// Creates a horizontal split with the current view and a new buffer.
	pub fn split_horizontal(&mut self, new_buffer_id: BufferId) {
		let current_view = self.focused_view;
		let new_layout = Layout::hsplit(Layout::single(current_view), Layout::text(new_buffer_id));
		self.layout.replace_view(current_view, new_layout);
		self.focus_buffer(new_buffer_id);
	}

	/// Creates a vertical split with the current view and a new buffer.
	pub fn split_vertical(&mut self, new_buffer_id: BufferId) {
		let current_view = self.focused_view;
		let new_layout = Layout::vsplit(Layout::single(current_view), Layout::text(new_buffer_id));
		self.layout.replace_view(current_view, new_layout);
		self.focus_buffer(new_buffer_id);
	}

	/// Opens a new terminal in a horizontal split.
	pub fn split_horizontal_terminal(&mut self) -> TerminalId {
		let terminal_id = self.create_terminal();
		let current_view = self.focused_view;
		let new_layout =
			Layout::hsplit(Layout::single(current_view), Layout::terminal(terminal_id));
		self.layout.replace_view(current_view, new_layout);
		self.focus_terminal(terminal_id);
		terminal_id
	}

	/// Opens a new terminal in a vertical split.
	pub fn split_vertical_terminal(&mut self) -> TerminalId {
		let terminal_id = self.create_terminal();
		let current_view = self.focused_view;
		let new_layout =
			Layout::vsplit(Layout::single(current_view), Layout::terminal(terminal_id));
		self.layout.replace_view(current_view, new_layout);
		self.focus_terminal(terminal_id);
		terminal_id
	}

	/// Creates a new terminal with IPC integration.
	fn create_terminal(&mut self) -> TerminalId {
		use tome_manifest::SplitBuffer;

		let terminal_id = TerminalId(self.next_terminal_id);
		self.next_terminal_id += 1;

		let ipc_env = self
			.terminal_ipc
			.get_or_insert_with(|| {
				let ipc = TerminalIpc::new().expect("failed to create terminal IPC");
				log::info!(
					"Terminal IPC created: bin_dir={:?}, socket={:?}",
					ipc.env().bin_dir(),
					ipc.env().socket_path()
				);
				ipc
			})
			.env();

		log::info!(
			"Creating terminal with PATH including {:?}",
			ipc_env.bin_dir()
		);

		let mut terminal = TerminalBuffer::with_ipc(ipc_env);
		terminal.on_open();
		self.terminals.insert(terminal_id, terminal);
		terminal_id
	}

	/// Polls for and dispatches IPC requests from embedded terminals.
	fn poll_terminal_ipc(&mut self) {
		let requests: Vec<_> = self
			.terminal_ipc
			.as_mut()
			.map(|ipc| std::iter::from_fn(|| ipc.poll()).collect())
			.unwrap_or_default();

		for req in requests {
			log::debug!("terminal IPC: {} {:?}", req.command, req.args);
			let mut restore_view = None;
			if self.is_terminal_focused()
				&& let Some(buffer_id) = self.layout.first_buffer()
			{
				restore_view = Some(self.focused_view);
				self.focused_view = BufferView::Text(buffer_id);
			}

			let outcome = if let Some(cmd) = tome_manifest::find_command(&req.command) {
				let args: Vec<&str> = req.args.iter().map(String::as_str).collect();
				let mut ctx = tome_manifest::CommandContext {
					editor: self,
					args: &args,
					count: 1,
					register: None,
					user_data: cmd.user_data,
				};
				let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
					tokio::task::block_in_place(|| handle.block_on((cmd.handler)(&mut ctx)))
				} else {
					futures::executor::block_on((cmd.handler)(&mut ctx))
				};
				match result {
					Ok(outcome) => outcome,
					Err(err) => {
						self.notify("error", err.to_string());
						tome_manifest::CommandOutcome::Ok
					}
				}
			} else {
				self.notify("error", format!("Unknown command: {}", req.command));
				tome_manifest::CommandOutcome::Ok
			};

			if let Some(view) = restore_view {
				self.focused_view = view;
			}

			self.handle_command_outcome(outcome);
		}
	}

	fn handle_command_outcome(&mut self, outcome: tome_manifest::CommandOutcome) {
		match outcome {
			tome_manifest::CommandOutcome::Ok => {}
			tome_manifest::CommandOutcome::Quit | tome_manifest::CommandOutcome::ForceQuit => {
				self.request_quit();
			}
		}
	}

	fn request_quit(&mut self) {
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

		// Remove from layout
		if let Some(new_layout) = self.layout.remove_view(view) {
			self.layout = new_layout;
		}

		// Remove the actual buffer/terminal
		match view {
			BufferView::Text(id) => {
				self.buffers.remove(&id);
			}
			BufferView::Terminal(id) => {
				self.terminals.remove(&id);
			}
		}

		// If we closed the focused view, focus another one
		if self.focused_view == view {
			self.focused_view = self.layout.first_view();
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
		self.close_view(self.focused_view)
	}

	/// Closes the current buffer if a text buffer is focused.
	///
	/// Returns true if the buffer was closed.
	pub fn close_current_buffer(&mut self) -> bool {
		match self.focused_view {
			BufferView::Text(id) => self.close_buffer(id),
			BufferView::Terminal(_) => false,
		}
	}

	pub fn mode(&self) -> Mode {
		if self.is_terminal_focused() {
			// Check if we're in window mode (using first buffer's input handler)
			if let Some(first_buffer_id) = self.layout.first_buffer()
				&& let Some(buffer) = self.buffers.get(&first_buffer_id)
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
				&& let Some(buffer) = self.buffers.get(&first_buffer_id)
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

		use tome_manifest::SplitBuffer;

		use crate::editor::extensions::TICK_EXTENSIONS;

		self.poll_terminal_ipc();

		// Tick all terminals
		let terminal_ids: Vec<_> = self.terminals.keys().copied().collect();
		for id in terminal_ids {
			if let Some(terminal) = self.terminals.get_mut(&id) {
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
		if let Some(anim) = &self.separator_hover_animation
			&& anim.needs_redraw()
		{
			self.needs_redraw = true;
		}
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
	}

	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		let yank = self.registers.yank.clone();
		self.buffer_mut().paste_before(&yank);
	}

	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.window_width = Some(width);
		self.window_height = Some(height);

		// Update text width for all buffers
		for buffer in self.buffers.values_mut() {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let mut ui = std::mem::take(&mut self.ui);
		ui.notify_resize(self, width, height);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
		self.needs_redraw = true;
	}

	pub fn handle_focus_in(&mut self) {
		self.needs_redraw = true;
	}

	pub fn handle_focus_out(&mut self) {
		self.needs_redraw = true;
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
		self.buffer_mut().delete_selection();
	}

	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), tome_manifest::CommandError> {
		if let Some(theme) = tome_theme::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = tome_theme::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(tome_manifest::CommandError::Failed(err))
		}
	}

	pub fn set_filesystem(&mut self, fs: Arc<dyn FileSystem>) {
		self.fs = fs;
	}

	pub fn collect_highlight_spans(
		&self,
		area: tome_tui::layout::Rect,
	) -> Vec<(
		tome_language::highlight::HighlightSpan,
		tome_tui::style::Style,
	)> {
		let buffer = self.buffer();
		let Some(ref syntax) = buffer.syntax else {
			return Vec::new();
		};

		let start_line = buffer.scroll_line;
		let end_line = (start_line + area.height as usize).min(buffer.doc.len_lines());

		let start_byte = buffer.doc.line_to_byte(start_line) as u32;
		let end_byte = if end_line < buffer.doc.len_lines() {
			buffer.doc.line_to_byte(end_line) as u32
		} else {
			buffer.doc.len_bytes() as u32
		};

		let highlight_styles =
			tome_language::highlight::HighlightStyles::new(SyntaxStyles::scope_names(), |scope| {
				self.theme.colors.syntax.resolve(scope)
			});

		let highlighter = syntax.highlighter(
			buffer.doc.slice(..),
			&self.language_loader,
			start_byte..end_byte,
		);

		highlighter
			.map(|span| {
				let abstract_style = highlight_styles.style_for_highlight(span.highlight);
				let tome_tui_style: tome_tui::style::Style = abstract_style.into();
				(span, tome_tui_style)
			})
			.collect()
	}

	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(
			tome_language::highlight::HighlightSpan,
			tome_tui::style::Style,
		)],
	) -> Option<tome_tui::style::Style> {
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
		style: Option<tome_tui::style::Style>,
	) -> Option<tome_tui::style::Style> {
		use tome_tui::animation::Animatable;

		use crate::editor::extensions::StyleMod;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to tome_tui color for blending
				let bg: tome_tui::style::Color = self.theme.colors.ui.bg.into();
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(tome_tui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => style.fg(color),
			StyleMod::Bg(color) => style.bg(color),
		};

		Some(modified)
	}

	pub fn apply_transaction(&mut self, tx: &Transaction) {
		let BufferView::Text(buffer_id) = self.focused_view else {
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_mut(&buffer_id)
			.expect("focused buffer must exist");
		buffer.apply_transaction_with_syntax(tx, &self.language_loader);
	}

	pub fn reparse_syntax(&mut self) {
		let BufferView::Text(buffer_id) = self.focused_view else {
			return;
		};

		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_mut(&buffer_id)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.language_loader);
	}

	fn path_to_virtual(&self, path: &Path) -> Option<String> {
		let cwd = std::env::current_dir().ok()?;
		Self::compute_virtual_path(path, &cwd)
	}
}
