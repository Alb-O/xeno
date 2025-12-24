mod acp;
mod actions;
mod actions_exec;
mod history;
mod input_handling;
mod messaging;
mod navigation;
mod plugins;
mod search;
pub mod types;

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use tome_core::ext::notifications::{Notifications, Overflow};
use tome_core::ext::{HookContext, emit_hook};
use tome_core::range::CharIdx;
use tome_core::{InputHandler, Mode, Rope, Selection, Transaction, ext, movement};
pub use types::{HistoryEntry, Message, MessageKind, Registers};

use crate::acp::AcpManager;
use crate::editor::types::CompletionState;
use crate::plugin::PluginManager;
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
	pub plugins: PluginManager,
	pub ui: UiManager,
	pub needs_redraw: bool,
	pub(crate) insert_undo_active: bool,
	pub(crate) pending_permissions: Vec<crate::plugin::manager::PendingPermission>,
	pub notifications: Notifications,
	pub last_tick: std::time::SystemTime,
	pub ipc: Option<crate::ipc::IpcServer>,
	pub completions: CompletionState,
	pub acp: AcpManager,
}

impl Editor {
	pub fn new(path: PathBuf) -> io::Result<Self> {
		let content = if path.exists() {
			fs::read_to_string(&path)?
		} else {
			String::new()
		};

		Ok(Self::from_content(content, Some(path)))
	}

	pub fn new_scratch() -> Self {
		Self::from_content(String::new(), None)
	}

	pub fn from_content(content: String, path: Option<PathBuf>) -> Self {
		let file_type = path
			.as_ref()
			.and_then(|p| ext::detect_file_type(p.to_str().unwrap_or("")))
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
			plugins: PluginManager::new(),
			ui: {
				let mut ui = UiManager::new();
				ui.register_panel(Box::new(crate::ui::panels::terminal::TerminalPanel::new()));
				ui.register_panel(Box::new(crate::ui::panels::plugins::PluginsPanel::new()));
				ui
			},
			needs_redraw: false,
			insert_undo_active: false,
			pending_permissions: Vec::new(),
			notifications: Notifications::new()
				.max_concurrent(Some(5))
				.overflow(Overflow::DiscardOldest),
			last_tick: std::time::SystemTime::now(),
			ipc: crate::ipc::IpcServer::start().ok(),
			completions: CompletionState::default(),
			acp: AcpManager::new(),
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

	pub fn save(&mut self) -> Result<(), tome_core::ext::CommandError> {
		let path_owned = match &self.path {
			Some(p) => p.clone(),
			None => {
				return Err(tome_core::ext::CommandError::InvalidArgument(
					"No filename. Use :write <filename>".to_string(),
				));
			}
		};

		emit_hook(&HookContext::BufferWritePre {
			path: &path_owned,
			text: self.doc.slice(..),
		});

		let mut f = fs::File::create(&path_owned)
			.map_err(|e| tome_core::ext::CommandError::Io(e.to_string()))?;
		for chunk in self.doc.chunks() {
			f.write_all(chunk.as_bytes())
				.map_err(|e| tome_core::ext::CommandError::Io(e.to_string()))?;
		}
		self.modified = false;
		self.show_message(format!("Saved {}", path_owned.display()));

		emit_hook(&HookContext::BufferWrite { path: &path_owned });

		Ok(())
	}

	pub fn save_as(&mut self, path: PathBuf) -> Result<(), tome_core::ext::CommandError> {
		self.path = Some(path);
		self.save()
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

	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), tome_core::ext::CommandError> {
		if let Some(theme) = crate::theme::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = crate::theme::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(tome_core::ext::CommandError::Failed(err))
		}
	}

	pub fn on_permission_decision(
		&mut self,
		request_id: u64,
		option_id: &str,
	) -> Result<(), tome_core::ext::CommandError> {
		let pos = self
			.pending_permissions
			.iter()
			.position(|p| p.request_id == request_id)
			.ok_or_else(|| {
				tome_core::ext::CommandError::Failed(format!(
					"No pending permission request with ID {}",
					request_id
				))
			})?;

		let pending = self.pending_permissions.remove(pos);
		let plugin_id = pending.plugin_id;

		if let Some(plugin) = self.plugins.plugins.get(&plugin_id)
			&& let Some(on_decision) = plugin.guest.on_permission_decision
		{
			use crate::plugin::manager::PluginContextGuard;
			let ed_ptr = self as *mut Editor;
			let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
			let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, &plugin_id) };
			let option_tome = tome_cabi_types::TomeStr {
				ptr: option_id.as_ptr(),
				len: option_id.len(),
			};
			on_decision(request_id, option_tome);
			return Ok(());
		}

		Err(tome_core::ext::CommandError::Failed(format!(
			"Plugin {} does not support permission decisions",
			plugin_id
		)))
	}

	pub fn plugin_command(&mut self, args: &[&str]) -> Result<(), tome_core::ext::CommandError> {
		if args.is_empty() {
			self.ui
				.toggle_panel(crate::ui::panels::plugins::PLUGINS_PANEL_ID);
			self.needs_redraw = true;
			return Ok(());
		}

		match args[0] {
			"enable" => {
				if args.len() < 2 {
					return Err(tome_core::ext::CommandError::MissingArgument("id"));
				}
				let id = args[1];
				if !self
					.plugins
					.config
					.plugins
					.enabled
					.contains(&id.to_string())
				{
					self.plugins.config.plugins.enabled.push(id.to_string());
					self.save_plugin_config();
				}
				let mgr_ptr = &mut self.plugins as *mut PluginManager;
				unsafe {
					(*mgr_ptr)
						.load(self, id)
						.map_err(|e| tome_core::ext::CommandError::Failed(e.to_string()))?
				};
				Ok(())
			}
			"disable" => {
				if args.len() < 2 {
					return Err(tome_core::ext::CommandError::MissingArgument("id"));
				}
				let id = args[1];
				self.plugins.config.plugins.enabled.retain(|e| e != id);
				self.save_plugin_config();
				self.show_message(format!("Plugin {} disabled. Restart to unload fully.", id));
				Ok(())
			}
			"reload" => {
				if args.len() < 2 {
					return Err(tome_core::ext::CommandError::MissingArgument("id"));
				}
				let id = args[1];
				let mgr_ptr = &mut self.plugins as *mut PluginManager;
				unsafe {
					(*mgr_ptr)
						.load(self, id)
						.map_err(|e| tome_core::ext::CommandError::Failed(e.to_string()))?
				};
				Ok(())
			}
			"logs" => {
				if args.len() < 2 {
					return Err(tome_core::ext::CommandError::MissingArgument("id"));
				}
				let id = args[1];
				let logs = self.plugins.logs.get(id).cloned().unwrap_or_default();
				let content = logs.join("\n");
				self.doc = Rope::from(content.as_str());
				self.path = Some(std::path::PathBuf::from(format!("plugin-logs-{}", id)));
				Ok(())
			}
			_ => Err(tome_core::ext::CommandError::Failed(format!(
				"Unknown plugin command: {}",
				args[0]
			))),
		}
	}
}
