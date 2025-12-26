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

use crate::buffer::{Buffer, BufferId, Layout};
use crate::editor::extensions::{EXTENSIONS, ExtensionMap, StyleOverlays};
use crate::editor::types::CompletionState;
use crate::render::{Notifications, Overflow};
use crate::ui::UiManager;

/// The main editor/workspace structure.
///
/// Contains one or more buffers and manages the workspace-level state
/// like theme, UI, notifications, and extensions.
pub struct Editor {
	/// All open buffers, keyed by BufferId.
	buffers: HashMap<BufferId, Buffer>,

	/// Counter for generating unique buffer IDs.
	next_buffer_id: u64,

	/// The currently focused buffer.
	focused_buffer: BufferId,

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
}

// Buffer access - provides convenient access to the focused buffer
impl Editor {
	/// Returns a reference to the currently focused buffer.
	#[inline]
	pub fn buffer(&self) -> &Buffer {
		self.buffers
			.get(&self.focused_buffer)
			.expect("focused buffer must exist")
	}

	/// Returns a mutable reference to the currently focused buffer.
	#[inline]
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		self.buffers
			.get_mut(&self.focused_buffer)
			.expect("focused buffer must exist")
	}

	/// Returns the ID of the focused buffer.
	pub fn focused_buffer_id(&self) -> BufferId {
		self.focused_buffer
	}

	/// Returns all buffer IDs.
	pub fn buffer_ids(&self) -> Vec<BufferId> {
		self.buffers.keys().copied().collect()
	}

	/// Returns a reference to a specific buffer by ID.
	pub fn get_buffer(&self, id: BufferId) -> Option<&Buffer> {
		self.buffers.get(&id)
	}

	/// Returns a mutable reference to a specific buffer by ID.
	pub fn get_buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
		self.buffers.get_mut(&id)
	}

	/// Returns the number of open buffers.
	pub fn buffer_count(&self) -> usize {
		self.buffers.len()
	}
}

impl tome_manifest::editor_ctx::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		self.buffer().modified
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), tome_manifest::CommandError>> + '_>,
	> {
		Box::pin(async move {
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
			next_buffer_id: 2, // Next ID will be 2
			focused_buffer: buffer_id,
			layout: Layout::single(buffer_id),
			message: None,
			registers: Registers::default(),
			theme: tome_theme::get_theme(tome_theme::DEFAULT_THEME_ID)
				.unwrap_or(&tome_theme::DEFAULT_THEME),
			window_width: None,
			window_height: None,
			ui: {
				use crate::terminal_buffer::TerminalBuffer;
				use crate::ui::{SplitBufferPanel, SplitBufferPanelConfig, UiKeyChord};

				let mut ui = UiManager::new();
				let terminal_config =
					SplitBufferPanelConfig::new("terminal").with_toggle(UiKeyChord::ctrl_char('t'));
				let terminal_panel = SplitBufferPanel::new(terminal_config, TerminalBuffer::new());
				ui.register_panel(Box::new(terminal_panel));
				ui
			},
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

	/// Focuses a specific buffer by ID.
	///
	/// Returns true if the buffer exists and was focused.
	pub fn focus_buffer(&mut self, id: BufferId) -> bool {
		if self.buffers.contains_key(&id) {
			self.focused_buffer = id;
			self.needs_redraw = true;
			true
		} else {
			false
		}
	}

	/// Focuses the next buffer in the layout.
	pub fn focus_next_buffer(&mut self) {
		let next_id = self.layout.next_buffer(self.focused_buffer);
		self.focus_buffer(next_id);
	}

	/// Focuses the previous buffer in the layout.
	pub fn focus_prev_buffer(&mut self) {
		let prev_id = self.layout.prev_buffer(self.focused_buffer);
		self.focus_buffer(prev_id);
	}

	/// Creates a horizontal split with the current buffer and a new buffer.
	pub fn split_horizontal(&mut self, new_buffer_id: BufferId) {
		let current_id = self.focused_buffer;
		let new_layout = Layout::hsplit(Layout::single(current_id), Layout::single(new_buffer_id));
		self.layout.replace(current_id, new_layout);
		self.focus_buffer(new_buffer_id);
	}

	/// Creates a vertical split with the current buffer and a new buffer.
	pub fn split_vertical(&mut self, new_buffer_id: BufferId) {
		let current_id = self.focused_buffer;
		let new_layout = Layout::vsplit(Layout::single(current_id), Layout::single(new_buffer_id));
		self.layout.replace(current_id, new_layout);
		self.focus_buffer(new_buffer_id);
	}

	/// Closes a buffer.
	///
	/// Returns true if the buffer was closed. Returns false if it's the last buffer
	/// (we don't allow closing the last buffer).
	pub fn close_buffer(&mut self, id: BufferId) -> bool {
		if self.buffers.len() <= 1 {
			return false;
		}

		// Remove from layout
		if let Some(new_layout) = self.layout.remove(id) {
			self.layout = new_layout;
		}

		// Remove the buffer
		self.buffers.remove(&id);

		// If we closed the focused buffer, focus another one
		if self.focused_buffer == id {
			self.focused_buffer = self.layout.first_buffer();
		}

		self.needs_redraw = true;
		true
	}

	/// Closes the current buffer.
	///
	/// Returns true if the buffer was closed.
	pub fn close_current_buffer(&mut self) -> bool {
		self.close_buffer(self.focused_buffer)
	}

	pub fn mode(&self) -> Mode {
		self.buffer().input.mode()
	}

	pub fn mode_name(&self) -> &'static str {
		self.buffer().input.mode_name()
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
		use crate::editor::extensions::TICK_EXTENSIONS;

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
		area: ratatui::layout::Rect,
	) -> Vec<(
		tome_language::highlight::HighlightSpan,
		ratatui::style::Style,
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
				let ratatui_style: ratatui::style::Style = abstract_style.into();
				(span, ratatui_style)
			})
			.collect()
	}

	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(
			tome_language::highlight::HighlightSpan,
			ratatui::style::Style,
		)],
	) -> Option<ratatui::style::Style> {
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
		style: Option<ratatui::style::Style>,
	) -> Option<ratatui::style::Style> {
		use tome_theme::blend_colors;

		use crate::editor::extensions::StyleMod;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				let bg = self.theme.colors.ui.bg;
				if let Some(ratatui::style::Color::Rgb(r, g, b)) = style.fg {
					let fg = tome_base::color::Color::Rgb(r, g, b);
					let dimmed = blend_colors(fg, bg, factor);
					let tome_base::color::Color::Rgb(dr, dg, db) = dimmed else {
						return Some(style);
					};
					style.fg(ratatui::style::Color::Rgb(dr, dg, db))
				} else {
					style.fg(ratatui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => {
				let ratatui_color: ratatui::style::Color = color.into();
				style.fg(ratatui_color)
			}
			StyleMod::Bg(color) => {
				let ratatui_color: ratatui::style::Color = color.into();
				style.bg(ratatui_color)
			}
		};

		Some(modified)
	}

	pub fn apply_transaction(&mut self, tx: &Transaction) {
		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_mut(&self.focused_buffer)
			.expect("focused buffer must exist");
		buffer.apply_transaction_with_syntax(tx, &self.language_loader);
	}

	pub fn reparse_syntax(&mut self) {
		// Access buffer directly to avoid borrow conflict with language_loader.
		let buffer = self
			.buffers
			.get_mut(&self.focused_buffer)
			.expect("focused buffer must exist");
		buffer.reparse_syntax(&self.language_loader);
	}

	fn path_to_virtual(&self, path: &Path) -> Option<String> {
		let cwd = std::env::current_dir().ok()?;
		Self::compute_virtual_path(path, &cwd)
	}
}
