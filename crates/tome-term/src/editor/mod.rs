mod actions;
mod actions_exec;
pub mod extensions;
mod history;
mod input_handling;
mod messaging;
mod navigation;
mod search;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tome_core::range::CharIdx;
use tome_core::registry::notifications::{Notifications, Overflow};
use tome_core::registry::{HookContext, emit_hook};
use tome_core::{InputHandler, Mode, Rope, Selection, Transaction, movement, registry};
pub use types::{HistoryEntry, Message, MessageKind, Registers};

use crate::editor::extensions::{EXTENSIONS, ExtensionMap};
use crate::editor::types::CompletionState;
use crate::theme::Theme;
use crate::ui::UiManager;

pub struct Editor {
	pub doc: Rope,
	pub cursor: CharIdx,
	pub selection: Selection,
	pub input: InputHandler,
	pub path: Option<PathBuf>,
	pub modified: bool,
	pub scroll_line: usize,
	pub scroll_segment: usize,
	pub message: Option<Message>,
	pub registers: Registers,
	pub undo_stack: Vec<HistoryEntry>,
	pub redo_stack: Vec<HistoryEntry>,
	pub text_width: usize,

	pub file_type: Option<String>,
	pub theme: &'static Theme,
	pub window_width: Option<u16>,
	pub window_height: Option<u16>,
	pub ui: UiManager,
	pub needs_redraw: bool,
	pub(crate) insert_undo_active: bool,
	pub notifications: Notifications,
	pub last_tick: std::time::SystemTime,
	#[allow(
		dead_code,
		reason = "IPC server currently only used for internal messaging, but field is read via debug tools"
	)]
	pub ipc: Option<crate::ipc::IpcServer>,
	pub completions: CompletionState,
	pub extensions: ExtensionMap,
	pub fs: Arc<dyn FileSystem>,
}

#[async_trait(?Send)]
impl tome_core::registry::EditorOps for Editor {
	fn path(&self) -> Option<&std::path::Path> {
		self.path.as_deref()
	}

	async fn save(&mut self) -> Result<(), tome_core::registry::CommandError> {
		let path_owned = match &self.path {
			Some(p) => p.clone(),
			None => {
				return Err(tome_core::registry::CommandError::InvalidArgument(
					"No filename. Use :write <filename>".to_string(),
				));
			}
		};

		emit_hook(&HookContext::BufferWritePre {
			path: &path_owned,
			text: self.doc.slice(..),
		});

		let mut content = Vec::new();
		for chunk in self.doc.chunks() {
			content.extend_from_slice(chunk.as_bytes());
		}

		self.fs
			.write_file(path_owned.to_str().unwrap_or(""), &content)
			.await
			.map_err(|e| tome_core::registry::CommandError::Io(e.to_string()))?;

		self.modified = false;
		self.show_message(format!("Saved {}", path_owned.display()));

		emit_hook(&HookContext::BufferWrite { path: &path_owned });

		Ok(())
	}

	async fn save_as(&mut self, path: PathBuf) -> Result<(), tome_core::registry::CommandError> {
		self.path = Some(path);
		self.save().await
	}

	fn insert_text(&mut self, text: &str) {
		self.insert_text(text);
	}

	fn delete_selection(&mut self) {
		self.delete_selection();
	}

	fn set_modified(&mut self, modified: bool) {
		self.modified = modified;
	}

	fn is_modified(&self) -> bool {
		self.modified
	}

	fn set_theme(&mut self, theme_name: &str) -> Result<(), tome_core::registry::CommandError> {
		self.set_theme(theme_name)
	}
}

impl Editor {
	pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
		let fs = Arc::new(HostFS::new(std::env::current_dir()?)?);
		let content = if fs.stat(path.to_str().unwrap_or("")).await?.is_some() {
			let bytes = fs
				.read_file(path.to_str().unwrap_or(""))
				.await?
				.unwrap_or_default();
			String::from_utf8_lossy(&bytes).to_string()
		} else {
			String::new()
		};

		Ok(Self::from_content(fs, content, Some(path)))
	}

	pub fn new_scratch() -> Self {
		let fs = Arc::new(HostFS::new(std::env::current_dir().unwrap()).unwrap());
		Self::from_content(fs, String::new(), None)
	}

	pub fn from_content(fs: Arc<dyn FileSystem>, content: String, path: Option<PathBuf>) -> Self {
		let file_type = path
			.as_ref()
			.and_then(|p| registry::detect_file_type(p.to_str().unwrap_or("")))
			.map(|ft| ft.name);

		let doc = Rope::from(content.as_str());

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);

		emit_hook(&HookContext::BufferOpen {
			path: hook_path,
			text: doc.slice(..),
			file_type,
		});

		Self {
			doc,
			cursor: 0,
			selection: Selection::point(0),
			input: InputHandler::new(),
			path,
			modified: false,
			scroll_line: 0,
			scroll_segment: 0,
			message: None,
			registers: Registers::default(),
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			text_width: 80,
			file_type: file_type.map(|s| s.to_string()),
			theme: &crate::themes::solarized::SOLARIZED_DARK,
			window_width: None,
			window_height: None,
			ui: {
				let mut ui = UiManager::new();
				ui.register_panel(Box::new(crate::ui::panels::terminal::TerminalPanel::new()));
				ui
			},
			needs_redraw: false,
			insert_undo_active: false,
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
		}
	}

	pub fn mode(&self) -> Mode {
		self.input.mode()
	}

	pub fn mode_name(&self) -> &'static str {
		self.input.mode_name()
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

	pub fn any_panel_open(&self) -> bool {
		self.ui.any_panel_open()
	}

	pub fn insert_text(&mut self, text: &str) {
		self.save_insert_undo_state();

		// Collapse all selections to their insertion points (line starts for ranges) so we insert at each cursor.
		let mut insertion_points = self.selection.clone();
		insertion_points.transform_mut(|r| {
			let pos = r.min();
			r.anchor = pos;
			r.head = pos;
		});

		let tx = Transaction::insert(self.doc.slice(..), &insertion_points, text.to_string());
		let mut new_selection = tx.map_selection(&insertion_points);
		new_selection.transform_mut(|r| {
			let pos = r.max();
			r.anchor = pos;
			r.head = pos;
		});
		tx.apply(&mut self.doc);

		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
		self.modified = true;
	}

	pub fn yank_selection(&mut self) {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			self.registers.yank = self.doc.slice(from..to).to_string();
			self.show_message(format!("Yanked {} chars", to - from));
		}
	}

	pub fn paste_after(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		let slice = self.doc.slice(..);
		self.selection.transform_mut(|r| {
			*r = movement::move_horizontally(
				slice,
				*r,
				tome_core::range::Direction::Forward,
				1,
				false,
			);
		});
		self.insert_text(&self.registers.yank.clone());
	}

	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		self.insert_text(&self.registers.yank.clone());
	}

	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.window_width = Some(width);
		self.window_height = Some(height);
		self.text_width = width.saturating_sub(self.gutter_width()) as usize;
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
		if !self.selection.primary().is_empty() {
			self.save_undo_state();
			let tx = Transaction::delete(self.doc.slice(..), &self.selection);
			self.selection = tx.map_selection(&self.selection);
			tx.apply(&mut self.doc);
			self.modified = true;
		}
	}

	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), tome_core::registry::CommandError> {
		if let Some(theme) = crate::theme::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = crate::theme::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(tome_core::registry::CommandError::Failed(err))
		}
	}

	pub fn set_filesystem(&mut self, fs: Arc<dyn FileSystem>) {
		self.fs = fs;
	}
}
