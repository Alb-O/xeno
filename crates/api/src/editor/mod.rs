mod actions;
mod actions_exec;
pub mod extensions;
mod history;
mod input_handling;
mod messaging;
mod navigation;
mod search;
pub mod types;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentfs_sdk::{FileSystem, HostFS};
use tome_base::Transaction;
use tome_language::LanguageLoader;
use tome_manifest::syntax::SyntaxStyles;
use tome_manifest::{HookContext, Mode, emit_hook};
use tome_theme::Theme;
pub use types::{HistoryEntry, Message, MessageKind, Registers};

use crate::buffer::{Buffer, BufferId, BufferView, Layout, SplitDirection, SplitPath, TerminalId};
use crate::editor::extensions::{EXTENSIONS, ExtensionMap, StyleOverlays};
use crate::editor::types::CompletionState;
use crate::render::{Notifications, Overflow};
use crate::terminal_buffer::TerminalBuffer;
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

	/// Workspace-level message (shown in status line).
	pub message: Option<Message>,

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

	/// Notification system.
	pub notifications: Notifications,

	/// Last tick timestamp.
	pub last_tick: std::time::SystemTime,

	/// IPC server for external communication.
	#[allow(
		dead_code,
		reason = "IPC server currently only used for internal messaging, but field is read via debug tools"
	)]
	pub ipc: Option<crate::ipc::IpcServer>,

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
}

/// State for an active separator drag operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DragState {
	/// Direction of the split being resized.
	pub direction: SplitDirection,
	/// Path to the split in the layout tree.
	pub path: SplitPath,
}

/// Tracks mouse velocity to determine if hover effects should be suppressed.
///
/// Fast mouse movement indicates the user is just passing through, not intending
/// to interact with separators. We suppress hover effects in this case to reduce
/// visual noise.
#[derive(Debug, Clone, Default)]
pub struct MouseVelocityTracker {
	/// Last known mouse position.
	last_position: Option<(u16, u16)>,
	/// When the last position was recorded.
	last_time: Option<std::time::Instant>,
	/// Smoothed velocity in cells per second.
	velocity: f32,
}

impl MouseVelocityTracker {
	/// Velocity threshold above which hover effects are suppressed (cells/second).
	const FAST_THRESHOLD: f32 = 60.0;

	/// Time after which velocity is considered zero (mouse is idle).
	const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

	/// Updates the tracker with a new mouse position and returns current velocity.
	pub fn update(&mut self, x: u16, y: u16) -> f32 {
		let now = std::time::Instant::now();

		if let (Some((lx, ly)), Some(lt)) = (self.last_position, self.last_time) {
			let dx = (x as f32 - lx as f32).abs();
			let dy = (y as f32 - ly as f32).abs();
			let distance = (dx * dx + dy * dy).sqrt();
			let dt = now.duration_since(lt).as_secs_f32();

			if dt > 0.0 && dt < 0.5 {
				// Ignore stale readings (> 500ms gap)
				let instant_velocity = distance / dt;
				// Exponential moving average for smoothing
				self.velocity = self.velocity * 0.6 + instant_velocity * 0.4;
			}
		}

		self.last_position = Some((x, y));
		self.last_time = Some(now);
		self.velocity
	}

	/// Returns true if the mouse is moving fast enough to suppress hover effects.
	///
	/// Accounts for idle time - if mouse hasn't moved recently, velocity is zero.
	pub fn is_fast(&self) -> bool {
		// If mouse has been idle, velocity is effectively zero
		if let Some(lt) = self.last_time
			&& lt.elapsed() > Self::IDLE_TIMEOUT
		{
			return false;
		}
		self.velocity > Self::FAST_THRESHOLD
	}

	/// Returns the current smoothed velocity, accounting for idle time.
	pub fn velocity(&self) -> f32 {
		if let Some(lt) = self.last_time
			&& lt.elapsed() > Self::IDLE_TIMEOUT
		{
			return 0.0;
		}
		self.velocity
	}
}

/// Animation state for separator hover effects.
///
/// Uses a `ToggleTween<f32>` internally for smooth fade in/out transitions.
#[derive(Debug, Clone)]
pub struct SeparatorHoverAnimation {
	/// The separator rectangle being animated.
	pub rect: tome_tui::layout::Rect,
	/// The hover intensity tween (0.0 = unhovered, 1.0 = fully hovered).
	tween: tome_tui::animation::ToggleTween<f32>,
}

impl SeparatorHoverAnimation {
	/// Duration of the hover fade animation.
	const FADE_DURATION: std::time::Duration = std::time::Duration::from_millis(120);

	/// Creates a new hover animation for the given separator.
	pub fn new(rect: tome_tui::layout::Rect, hovering: bool) -> Self {
		let mut tween = tome_tui::animation::ToggleTween::new(0.0f32, 1.0f32, Self::FADE_DURATION)
			.with_easing(tome_tui::animation::Easing::EaseOut);
		tween.set_active(hovering);
		Self { rect, tween }
	}

	/// Creates a new hover animation starting at a specific intensity.
	///
	/// This is useful for creating fade-out animations that should start
	/// from a fully hovered state (intensity 1.0).
	pub fn new_at_intensity(rect: tome_tui::layout::Rect, intensity: f32, hovering: bool) -> Self {
		let tween = tome_tui::animation::ToggleTween::new_at(
			0.0f32,
			1.0f32,
			Self::FADE_DURATION,
			intensity,
			hovering,
		)
		.with_easing(tome_tui::animation::Easing::EaseOut);
		Self { rect, tween }
	}

	/// Returns whether we're animating toward hovered state.
	pub fn hovering(&self) -> bool {
		self.tween.is_active()
	}

	/// Sets the hover state, returning true if state changed.
	pub fn set_hovering(&mut self, hovering: bool) -> bool {
		self.tween.set_active(hovering)
	}

	/// Returns the effective hover intensity (0.0 = unhovered, 1.0 = fully hovered).
	pub fn intensity(&self) -> f32 {
		self.tween.value()
	}

	/// Returns true if the animation is complete.
	pub fn is_complete(&self) -> bool {
		self.tween.is_complete()
	}

	/// Returns true if the animation is still in progress.
	pub fn needs_redraw(&self) -> bool {
		self.tween.is_running()
	}
}

// Buffer and terminal access - provides convenient access to the focused view
impl Editor {
	/// Returns a reference to the currently focused text buffer.
	///
	/// Panics if the focused view is a terminal.
	#[inline]
	pub fn buffer(&self) -> &Buffer {
		match self.focused_view {
			BufferView::Text(id) => self.buffers.get(&id).expect("focused buffer must exist"),
			BufferView::Terminal(_) => panic!("focused view is a terminal, not a text buffer"),
		}
	}

	/// Returns a mutable reference to the currently focused text buffer.
	///
	/// Panics if the focused view is a terminal.
	#[inline]
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		match self.focused_view {
			BufferView::Text(id) => self
				.buffers
				.get_mut(&id)
				.expect("focused buffer must exist"),
			BufferView::Terminal(_) => panic!("focused view is a terminal, not a text buffer"),
		}
	}

	/// Returns the currently focused view.
	pub fn focused_view(&self) -> BufferView {
		self.focused_view
	}

	/// Returns true if the focused view is a text buffer.
	pub fn is_text_focused(&self) -> bool {
		self.focused_view.is_text()
	}

	/// Returns true if the focused view is a terminal.
	pub fn is_terminal_focused(&self) -> bool {
		self.focused_view.is_terminal()
	}

	/// Returns the ID of the focused text buffer, if one is focused.
	pub fn focused_buffer_id(&self) -> Option<BufferId> {
		self.focused_view.as_text()
	}

	/// Returns the ID of the focused terminal, if one is focused.
	pub fn focused_terminal_id(&self) -> Option<TerminalId> {
		self.focused_view.as_terminal()
	}

	/// Returns all text buffer IDs.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.buffers.keys().copied().collect()
	}

	/// Returns all terminal IDs.
	pub fn terminal_ids(&self) -> Vec<TerminalId> {
		self.terminals.keys().copied().collect()
	}

	/// Returns a reference to a specific buffer by ID.
	pub fn get_buffer(&self, id: BufferId) -> Option<&Buffer> {
		self.buffers.get(&id)
	}

	/// Returns a mutable reference to a specific buffer by ID.
	pub fn get_buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
		self.buffers.get_mut(&id)
	}

	/// Returns a reference to a specific terminal by ID.
	pub fn get_terminal(&self, id: TerminalId) -> Option<&TerminalBuffer> {
		self.terminals.get(&id)
	}

	/// Returns a mutable reference to a specific terminal by ID.
	pub fn get_terminal_mut(&mut self, id: TerminalId) -> Option<&mut TerminalBuffer> {
		self.terminals.get_mut(&id)
	}

	/// Returns the number of open text buffers.
	pub fn buffer_count(&self) -> usize {
		self.buffers.len()
	}

	/// Returns the number of open terminals.
	pub fn terminal_count(&self) -> usize {
		self.terminals.len()
	}
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
			});

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

			emit_hook(&HookContext::BufferWrite { path: &path_owned });

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

		emit_hook(&HookContext::BufferOpen {
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
			message: None,
			registers: Registers::default(),
			theme: tome_theme::get_theme(tome_theme::DEFAULT_THEME_ID)
				.unwrap_or(&tome_theme::DEFAULT_THEME),
			window_width: None,
			window_height: None,
			ui: UiManager::new(),
			needs_redraw: false,
			notifications: Notifications::new()
				.max_concurrent(Some(5))
				.overflow(Overflow::DiscardOldest),
			last_tick: std::time::SystemTime::now(),
			ipc: crate::ipc::IpcServer::start().ok(),
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

		emit_hook(&HookContext::BufferOpen {
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
		use tome_manifest::SplitBuffer;

		let terminal_id = TerminalId(self.next_terminal_id);
		self.next_terminal_id += 1;

		let mut terminal = TerminalBuffer::new();
		terminal.on_open(); // Start prewarming the terminal
		self.terminals.insert(terminal_id, terminal);

		let current_view = self.focused_view;
		let new_layout =
			Layout::hsplit(Layout::single(current_view), Layout::terminal(terminal_id));
		self.layout.replace_view(current_view, new_layout);
		self.focus_terminal(terminal_id);
		terminal_id
	}

	/// Opens a new terminal in a vertical split.
	pub fn split_vertical_terminal(&mut self) -> TerminalId {
		use tome_manifest::SplitBuffer;

		let terminal_id = TerminalId(self.next_terminal_id);
		self.next_terminal_id += 1;

		let mut terminal = TerminalBuffer::new();
		terminal.on_open(); // Start prewarming the terminal
		self.terminals.insert(terminal_id, terminal);

		let current_view = self.focused_view;
		let new_layout =
			Layout::vsplit(Layout::single(current_view), Layout::terminal(terminal_id));
		self.layout.replace_view(current_view, new_layout);
		self.focus_terminal(terminal_id);
		terminal_id
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
