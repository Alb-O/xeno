mod actions;
mod navigation;
mod search;
pub mod types;

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;
use std::{fs, mem};

use ratatui_notifications::{
	Anchor, Animation, Level, Notification, Notifications, Overflow, SizeConstraint, Timing,
};
use tome_cabi_types::{TomeCommandContextV1, TomeStatus, TomeStr};
use tome_core::ext::{HookContext, emit_hook};
use tome_core::key::{KeyCode, SpecialKey};
use tome_core::range::Direction as MoveDir;
use tome_core::{
	InputHandler, Key, KeyResult, Mode, MouseEvent, Rope, Selection, Transaction, ext, movement,
};
pub use types::{HistoryEntry, Message, MessageKind, Registers, ScratchState};

use crate::plugins::PluginManager;
use crate::plugins::manager::HOST_V2;
use crate::terminal_panel::TerminalState;
use crate::theme::Theme;

pub struct Editor {
	pub doc: Rope,
	pub cursor: usize,
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
	pub scratch: ScratchState,
	pub scratch_open: bool,
	pub scratch_keep_open: bool,
	pub scratch_focused: bool,

	pub terminal: Option<TerminalState>,
	terminal_prewarm: Option<Receiver<Result<TerminalState, String>>>,
	terminal_input_buffer: Vec<u8>,
	pub terminal_open: bool,
	pub terminal_focused: bool,
	terminal_focus_pending: bool,

	in_scratch_context: bool,
	pub file_type: Option<String>,
	pub theme: &'static Theme,
	pub window_width: Option<u16>,
	pub window_height: Option<u16>,
	pub plugins: PluginManager,
	pub needs_redraw: bool,
	insert_undo_active: bool,
	pub pending_permissions: Vec<crate::plugins::manager::PendingPermission>,
	pub notifications: Notifications,
	pub last_tick: std::time::SystemTime,
}

impl Editor {
	pub fn request_redraw(&mut self) {
		self.needs_redraw = true;
	}

	pub fn show_message(&mut self, text: impl Into<String>) {
		let text = text.into();
		self.message = Some(Message {
			text: text.clone(),
			kind: MessageKind::Info,
		});

		let style = ratatui::style::Style::default()
			.bg(self.theme.colors.popup.bg)
			.fg(self.theme.colors.popup.fg);

		if let Ok(notif) = Notification::new(text)
			.level(Level::Info)
			.animation(Animation::Fade)
			.anchor(Anchor::BottomRight)
			.timing(
				Timing::Fixed(Duration::from_millis(200)),
				Timing::Fixed(Duration::from_secs(3)),
				Timing::Fixed(Duration::from_millis(200)),
			)
			.max_size(SizeConstraint::Absolute(40), SizeConstraint::Absolute(5))
			.style(style)
			.build()
		{
			let _ = self.notifications.add(notif);
		}
	}

	pub fn show_error(&mut self, text: impl Into<String>) {
		let text = text.into();
		self.message = Some(Message {
			text: text.clone(),
			kind: MessageKind::Error,
		});

		let style = ratatui::style::Style::default()
			.bg(self.theme.colors.popup.bg)
			.fg(self.theme.colors.popup.fg);

		if let Ok(notif) = Notification::new(text)
			.level(Level::Error)
			.animation(Animation::Fade)
			.anchor(Anchor::BottomRight)
			.timing(
				Timing::Fixed(Duration::from_millis(200)),
				Timing::Fixed(Duration::from_secs(5)),
				Timing::Fixed(Duration::from_millis(200)),
			)
			.max_size(SizeConstraint::Absolute(40), SizeConstraint::Absolute(5))
			.style(style)
			.build()
		{
			let _ = self.notifications.add(notif);
		}
	}

	pub fn execute_ex_command(&mut self, input: &str) -> bool {
		let input = input.trim();
		let input = input.strip_prefix(':').unwrap_or(input);
		self.execute_command_line(input)
	}

	fn execute_command_line(&mut self, input: &str) -> bool {
		use ext::{CommandContext, CommandOutcome, find_command};

		let trimmed = input.trim();
		if trimmed.is_empty() {
			return false;
		}

		let mut parts = trimmed.split_whitespace();
		let name = match parts.next() {
			Some(n) => n,
			None => return false,
		};

		let arg_strings: Vec<String> = parts.map(|s| s.to_string()).collect();
		let args: Vec<&str> = arg_strings.iter().map(|s| s.as_str()).collect();

		if self.try_execute_plugin_command(name, &args) {
			return false;
		}

		let cmd = match find_command(name) {
			Some(cmd) => cmd,
			None => {
				self.show_error(format!("Unknown command: {}", name));
				return false;
			}
		};

		let mut ctx = CommandContext {
			editor: self,
			args: &args,
			count: 1,
			register: None,
		};

		match (cmd.handler)(&mut ctx) {
			Ok(CommandOutcome::Ok) => false,
			Ok(CommandOutcome::Quit) => true,
			Ok(CommandOutcome::ForceQuit) => true,
			Err(e) => {
				ctx.editor.error(&e.to_string());
				false
			}
		}
	}
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
			scratch: ScratchState::default(),
			scratch_open: false,
			scratch_keep_open: true,
			scratch_focused: false,
			terminal: None,
			terminal_prewarm: None,
			terminal_input_buffer: Vec::new(),
			terminal_open: false,
			terminal_focused: false,
			terminal_focus_pending: false,
			in_scratch_context: false,
			file_type: file_type.map(|s| s.to_string()),
			theme: &crate::themes::solarized::SOLARIZED_DARK,
			window_width: None,
			window_height: None,
			plugins: PluginManager::new(),
			needs_redraw: false,
			insert_undo_active: false,
			pending_permissions: Vec::new(),
			notifications: Notifications::new()
				.max_concurrent(Some(5))
				.overflow(Overflow::DiscardOldest),
			last_tick: std::time::SystemTime::now(),
		}
	}

	pub fn mode(&self) -> Mode {
		if self.in_scratch_context {
			return self.input.mode();
		}
		if self.scratch_focused {
			return self.scratch.input.mode();
		}
		self.input.mode()
	}

	pub(crate) fn in_scratch_context(&self) -> bool {
		self.in_scratch_context
	}

	pub fn mode_name(&self) -> &'static str {
		if self.in_scratch_context {
			return self.input.mode_name();
		}
		if self.scratch_focused {
			return self.scratch.input.mode_name();
		}
		self.input.mode_name()
	}

	pub(crate) fn enter_scratch_context(&mut self) {
		if self.in_scratch_context {
			return;
		}
		self.in_scratch_context = true;
		mem::swap(&mut self.doc, &mut self.scratch.doc);
		mem::swap(&mut self.cursor, &mut self.scratch.cursor);
		mem::swap(&mut self.selection, &mut self.scratch.selection);
		mem::swap(&mut self.input, &mut self.scratch.input);
		mem::swap(&mut self.path, &mut self.scratch.path);
		mem::swap(&mut self.modified, &mut self.scratch.modified);
		mem::swap(&mut self.scroll_line, &mut self.scratch.scroll_line);
		mem::swap(&mut self.scroll_segment, &mut self.scratch.scroll_segment);
		mem::swap(&mut self.undo_stack, &mut self.scratch.undo_stack);
		mem::swap(&mut self.redo_stack, &mut self.scratch.redo_stack);
		mem::swap(&mut self.text_width, &mut self.scratch.text_width);
		mem::swap(
			&mut self.insert_undo_active,
			&mut self.scratch.insert_undo_active,
		);
	}

	pub(crate) fn leave_scratch_context(&mut self) {
		if !self.in_scratch_context {
			return;
		}
		self.in_scratch_context = false;
		mem::swap(&mut self.doc, &mut self.scratch.doc);
		mem::swap(&mut self.cursor, &mut self.scratch.cursor);
		mem::swap(&mut self.selection, &mut self.scratch.selection);
		mem::swap(&mut self.input, &mut self.scratch.input);
		mem::swap(&mut self.path, &mut self.scratch.path);
		mem::swap(&mut self.modified, &mut self.scratch.modified);
		mem::swap(&mut self.scroll_line, &mut self.scratch.scroll_line);
		mem::swap(&mut self.scroll_segment, &mut self.scratch.scroll_segment);
		mem::swap(&mut self.undo_stack, &mut self.scratch.undo_stack);
		mem::swap(&mut self.redo_stack, &mut self.scratch.redo_stack);
		mem::swap(&mut self.text_width, &mut self.scratch.text_width);
		mem::swap(
			&mut self.insert_undo_active,
			&mut self.scratch.insert_undo_active,
		);
	}

	pub(crate) fn with_scratch_context<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
		self.enter_scratch_context();
		let result = f(self);
		self.leave_scratch_context();
		result
	}

	pub(crate) fn do_open_scratch(&mut self, focus: bool) {
		self.scratch_open = true;
		if focus {
			self.scratch_focused = true;
			self.with_scratch_context(|ed| {
				if ed.doc.len_chars() == 0 {
					ed.cursor = 0;
					ed.selection = Selection::point(0);
				}
				ed.input.set_mode(Mode::Insert);
			});
		}
	}

	pub(crate) fn do_close_scratch(&mut self) {
		if self.in_scratch_context {
			self.leave_scratch_context();
		}
		self.scratch_open = false;
		self.scratch_focused = false;
	}

	pub(crate) fn start_terminal_prewarm(&mut self) {
		if self.terminal.is_some() || self.terminal_prewarm.is_some() {
			return;
		}

		let (tx, rx) = std::sync::mpsc::channel();
		self.terminal_prewarm = Some(rx);

		std::thread::spawn(move || {
			let _ = tx.send(TerminalState::new(80, 24));
		});
	}

	pub(crate) fn poll_terminal_prewarm(&mut self) {
		let recv = match self.terminal_prewarm.as_ref() {
			Some(rx) => rx.try_recv(),
			None => return,
		};

		match recv {
			Ok(Ok(mut term)) => {
				// Flush any buffered keystrokes typed while the terminal was opening.
				if !self.terminal_input_buffer.is_empty() {
					let _ = term.write_key(&self.terminal_input_buffer);
					self.terminal_input_buffer.clear();
				}

				self.terminal = Some(term);
				self.terminal_prewarm = None;

				if self.terminal_open && self.terminal_focus_pending {
					// Keep terminal focused unless the user explicitly unfocused it while loading.
					self.terminal_focused = true;
					self.terminal_focus_pending = false;
				}
			}
			Ok(Err(e)) => {
				self.show_error(format!("Failed to start terminal: {}", e));
				self.terminal_prewarm = None;
				self.terminal_focus_pending = false;
			}
			Err(TryRecvError::Empty) => {}
			Err(TryRecvError::Disconnected) => {
				self.terminal_prewarm = None;
				self.terminal_focus_pending = false;
			}
		}
	}

	pub(crate) fn on_terminal_exit(&mut self) {
		self.terminal_open = false;
		self.terminal_focused = false;
		self.terminal_focus_pending = false;
		self.terminal_input_buffer.clear();
		self.terminal = None;

		// Keep a fresh shell ready for the next toggle.
		self.start_terminal_prewarm();
	}

	pub(crate) fn do_toggle_terminal(&mut self) {
		// Always poll in case the prewarm completed since the last frame.
		self.poll_terminal_prewarm();

		if self.terminal_open {
			if self.terminal_focused {
				self.terminal_open = false;
				self.terminal_focused = false;
				self.terminal_focus_pending = false;
				self.terminal_input_buffer.clear();
			} else if self.terminal.is_some() {
				self.terminal_focused = true;
				self.terminal_focus_pending = false;
			} else {
				self.start_terminal_prewarm();
				self.terminal_focus_pending = true;
			}
			return;
		}

		// Opening.
		self.terminal_open = true;
		if self.terminal.is_some() {
			self.terminal_focused = true;
			self.terminal_focus_pending = false;
		} else {
			self.start_terminal_prewarm();
			self.terminal_focused = true;
			self.terminal_focus_pending = true;
		}
	}

	pub(crate) fn do_toggle_scratch(&mut self) {
		if !self.scratch_open {
			self.do_open_scratch(true);
		} else if self.scratch_focused {
			self.do_close_scratch();
		} else {
			self.scratch_focused = true;
		}
	}

	pub fn submit_plugin_panel(&mut self, id: u64) {
		use tome_cabi_types::{TomeChatRole, TomeStr};

		use crate::plugins::panels::ChatItem;

		if let Some(panel) = self.plugins.panels.get_mut(&id) {
			let text = panel.input.to_string();
			if text.trim().is_empty() {
				return;
			}

			panel.transcript.push(ChatItem {
				role: TomeChatRole::User,
				text: text.clone(),
			});
			panel.input = "".into();
			panel.input_cursor = 0;

			let text_tome = TomeStr {
				ptr: text.as_ptr(),
				len: text.len(),
			};

			if let Some(owner_idx) = self.plugins.panel_owners.get(&id).copied()
				&& let Some(plugin) = self.plugins.plugins.get(owner_idx)
				&& let Some(on_submit) = plugin.guest.on_panel_submit
			{
				use crate::plugins::manager::PluginContextGuard;
				let ed_ptr = self as *mut Editor;
				let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
				let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, owner_idx) };
				on_submit(id, text_tome);
			}
		}
	}

	pub fn try_execute_plugin_command(&mut self, full_name: &str, args: &[&str]) -> bool {
		let cmd = match self.plugins.commands.get(full_name) {
			Some(c) => c,
			None => return false,
		};

		let plugin_idx = cmd.plugin_idx;
		let handler = cmd.handler;

		let arg_tome_strs: Vec<TomeStr> = args
			.iter()
			.map(|s| TomeStr {
				ptr: s.as_ptr(),
				len: s.len(),
			})
			.collect();

		let mut ctx = TomeCommandContextV1 {
			argc: args.len(),
			argv: arg_tome_strs.as_ptr(),
			host: &HOST_V2,
		};

		let status = {
			use crate::plugins::manager::PluginContextGuard;
			let ed_ptr = self as *mut Editor;
			let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
			let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, plugin_idx) };
			handler(&mut ctx)
		};

		if status != TomeStatus::Ok {
			self.show_error(format!(
				"Command {} failed with status {:?}",
				full_name, status
			));
		}
		true
	}

	pub fn autoload_plugins(&mut self) {
		let mgr_ptr = &mut self.plugins as *mut PluginManager;
		unsafe { (*mgr_ptr).autoload(self) };
	}

	pub fn poll_plugins(&mut self) {
		use crate::plugins::manager::PluginContextGuard;
		let mut events = Vec::new();
		let num_plugins = self.plugins.plugins.len();
		for idx in 0..num_plugins {
			if let Some(poll_event) = self.plugins.plugins[idx].guest.poll_event {
				let ed_ptr = self as *mut Editor;
				let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
				let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, idx) };
				loop {
					let mut event =
						std::mem::MaybeUninit::<tome_cabi_types::TomePluginEventV1>::uninit();
					let has_event = poll_event(event.as_mut_ptr());
					if has_event.0 == 0 {
						break;
					}
					let event = unsafe { event.assume_init() };
					events.push((idx, event));
				}
			}
		}

		for (idx, event) in events {
			self.handle_plugin_event(idx, event);
		}
	}

	fn handle_plugin_event(
		&mut self,
		plugin_idx: usize,
		event: tome_cabi_types::TomePluginEventV1,
	) {
		use tome_cabi_types::TomePluginEventKind;

		use crate::plugins::manager::tome_owned_to_string;
		use crate::plugins::panels::ChatItem;

		let free_str_fn = self.plugins.plugins[plugin_idx].guest.free_str;

		match event.kind {
			TomePluginEventKind::PanelAppend => {
				if self.plugins.panel_owners.get(&event.panel_id) == Some(&plugin_idx)
					&& let Some(panel) = self.plugins.panels.get_mut(&event.panel_id)
					&& let Some(text) = tome_owned_to_string(event.text)
				{
					panel.transcript.push(ChatItem {
						role: event.role,
						text,
					});
				}
			}
			TomePluginEventKind::PanelSetOpen => {
				if self.plugins.panel_owners.get(&event.panel_id) == Some(&plugin_idx)
					&& let Some(panel) = self.plugins.panels.get_mut(&event.panel_id)
				{
					panel.open = event.bool_val.0 != 0;
				}
			}
			TomePluginEventKind::ShowMessage => {
				if let Some(text) = tome_owned_to_string(event.text) {
					self.show_message(text);
				}
			}
			TomePluginEventKind::RequestPermission => {
				let req = unsafe { &*event.permission_request };
				let prompt = tome_owned_to_string(req.prompt).unwrap_or_default();
				let options_slice =
					unsafe { std::slice::from_raw_parts(req.options, req.options_len) };
				let mut options = Vec::new();
				for opt in options_slice {
					options.push((
						tome_owned_to_string(opt.option_id).unwrap_or_default(),
						tome_owned_to_string(opt.label).unwrap_or_default(),
					));
				}

				self.pending_permissions
					.push(crate::plugins::manager::PendingPermission {
						plugin_idx,
						request_id: event.permission_request_id,
						_prompt: prompt.clone(),
						_options: options.clone(),
					});

				self.show_message(format!(
					"Permission requested: {}. Use :permit {} <option>",
					prompt, event.permission_request_id,
				));
			}
		}

		if let Some(free_str) = free_str_fn
			&& !event.text.ptr.is_null()
		{
			free_str(event.text);
		}

		if !event.permission_request.is_null()
			&& let Some(free_perm) = self.plugins.plugins[plugin_idx]
				.guest
				.free_permission_request
		{
			free_perm(event.permission_request);
		}
	}

	pub(crate) fn do_execute_scratch(&mut self) -> bool {
		if !self.scratch_open {
			self.show_error("Scratch is not open");
			return false;
		}

		let text = self.with_scratch_context(|ed| ed.doc.slice(..).to_string());
		let flattened = text
			.lines()
			.map(str::trim_end)
			.filter(|l| !l.is_empty())
			.collect::<Vec<_>>()
			.join(" ");

		let trimmed = flattened.trim();
		if trimmed.is_empty() {
			self.show_error("Scratch buffer is empty");
			return false;
		}

		let command = if let Some(stripped) = trimmed.strip_prefix(':') {
			stripped.trim_start()
		} else {
			trimmed
		};

		// Alias 'exit' to 'quit' if needed, or just rely on execute_command_line
		if command == "exit" {
			return true;
		}

		let result = self.execute_command_line(command);

		if !self.scratch_keep_open {
			self.do_close_scratch();
		}

		result
	}

	fn push_undo_snapshot(&mut self) {
		self.undo_stack.push(HistoryEntry {
			doc: self.doc.clone(),
			selection: self.selection.clone(),
		});
		self.redo_stack.clear();

		const MAX_UNDO: usize = 100;
		if self.undo_stack.len() > MAX_UNDO {
			self.undo_stack.remove(0);
		}
	}

	pub fn save_undo_state(&mut self) {
		// Explicit calls reset any grouped insert session.
		self.insert_undo_active = false;
		self.push_undo_snapshot();
	}

	fn save_insert_undo_state(&mut self) {
		if self.insert_undo_active {
			return;
		}
		self.insert_undo_active = true;
		self.push_undo_snapshot();
	}

	pub fn undo(&mut self) {
		self.insert_undo_active = false;
		if let Some(entry) = self.undo_stack.pop() {
			self.redo_stack.push(HistoryEntry {
				doc: self.doc.clone(),
				selection: self.selection.clone(),
			});

			self.doc = entry.doc;
			self.selection = entry.selection;
			self.show_message("Undo");
		} else {
			self.show_message("Nothing to undo");
		}
	}

	pub fn redo(&mut self) {
		self.insert_undo_active = false;
		if let Some(entry) = self.redo_stack.pop() {
			self.undo_stack.push(HistoryEntry {
				doc: self.doc.clone(),
				selection: self.selection.clone(),
			});

			self.doc = entry.doc;
			self.selection = entry.selection;
			self.show_message("Redo");
		} else {
			self.show_message("Nothing to redo");
		}
	}

	pub fn insert_text(&mut self, text: &str) {
		self.save_insert_undo_state();

		// Collapse all selections to their insertion points (line starts for ranges) so we insert at each cursor.
		let mut insertion_points = self.selection.clone();
		insertion_points.transform_mut(|r| {
			let pos = r.from();
			r.anchor = pos;
			r.head = pos;
		});

		let tx = Transaction::insert(self.doc.slice(..), &insertion_points, text.to_string());
		let mut new_selection = tx.map_selection(&insertion_points);
		new_selection.transform_mut(|r| {
			let pos = r.to();
			r.anchor = pos;
			r.head = pos;
		});
		tx.apply(&mut self.doc);

		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
		self.modified = true;
	}

	pub fn save(&mut self) -> io::Result<()> {
		let path_owned = match &self.path {
			Some(p) => p.clone(),
			None => {
				return Err(io::Error::new(
					io::ErrorKind::InvalidInput,
					"No filename. Use :write <filename>",
				));
			}
		};

		emit_hook(&HookContext::BufferWritePre {
			path: &path_owned,
			text: self.doc.slice(..),
		});

		let mut f = fs::File::create(&path_owned)?;
		for chunk in self.doc.chunks() {
			f.write_all(chunk.as_bytes())?;
		}
		self.modified = false;
		self.show_message(format!("Saved {}", path_owned.display()));

		emit_hook(&HookContext::BufferWrite { path: &path_owned });

		Ok(())
	}

	pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
		self.path = Some(path);
		self.save()
	}

	pub fn yank_selection(&mut self) {
		let primary = self.selection.primary();
		let from = primary.from();
		let to = primary.to();
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
			*r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
		});
		self.insert_text(&self.registers.yank.clone());
	}

	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		self.insert_text(&self.registers.yank.clone());
	}

	pub fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		use termina::event::{KeyCode as TmKeyCode, Modifiers as TmModifiers};

		// Toggle terminal with Ctrl+` (or similar, but let's just use a command for now,
		// wait, I can bind a key here or rely on command.
		// Let's add a check for a specific toggle key globally or just handle focus)
		// Actually, keybinding system in tome-core handles global keys.
		// But if terminal is focused, we swallow keys.
		// We need a way to toggle terminal even if focused.
		// Let's say Ctrl+t toggles terminal for now (hardcoded)
		if matches!(key.code, TmKeyCode::Char('t')) && key.modifiers.contains(TmModifiers::CONTROL)
		{
			self.do_toggle_terminal();
			return false;
		}

		if self.terminal_open && self.terminal_focused {
			// Esc to exit terminal focus (but keep open)
			if matches!(key.code, TmKeyCode::Escape) {
				self.terminal_focused = false;
				self.terminal_focus_pending = false;
				self.terminal_input_buffer.clear();
				return false;
			}

			// Convert key -> terminal bytes.
			let bytes = match key.code {
				TmKeyCode::Char(c) => {
					if key.modifiers.contains(TmModifiers::CONTROL) {
						let byte = c.to_ascii_lowercase() as u8;
						if byte.is_ascii_lowercase() {
							vec![byte - b'a' + 1]
						} else {
							vec![byte]
						}
					} else {
						let mut b = [0; 4];
						c.encode_utf8(&mut b).as_bytes().to_vec()
					}
				}
				TmKeyCode::Enter => vec![b'\r'],
				TmKeyCode::Backspace => vec![0x7f],
				TmKeyCode::Tab => vec![b'\t'],
				TmKeyCode::Up => b"\x1b[A".to_vec(),
				TmKeyCode::Down => b"\x1b[B".to_vec(),
				TmKeyCode::Right => b"\x1b[C".to_vec(),
				TmKeyCode::Left => b"\x1b[D".to_vec(),
				_ => vec![],
			};

			if !bytes.is_empty() {
				if let Some(term) = &mut self.terminal {
					let _ = term.write_key(&bytes);
				} else {
					// Terminal is still starting: buffer until the prewarm completes.
					self.terminal_input_buffer.extend_from_slice(&bytes);
				}
			}

			return false;
		}

		// Check plugin panels
		let mut panel_id_to_submit = None;
		let mut panel_handled = false;
		for panel in self.plugins.panels.values_mut() {
			if panel.open && panel.focused {
				let raw_ctrl_enter = matches!(
					key.code,
					TmKeyCode::Enter | TmKeyCode::Char('\n') | TmKeyCode::Char('j')
				) && key.modifiers.contains(TmModifiers::CONTROL);

				if raw_ctrl_enter {
					panel_id_to_submit = Some(panel.id);
				} else {
					match key.code {
						TmKeyCode::Char(c) => {
							panel.input.insert(panel.input_cursor, &c.to_string());
							panel.input_cursor += 1;
						}
						TmKeyCode::Backspace => {
							if panel.input_cursor > 0 {
								panel
									.input
									.remove(panel.input_cursor - 1..panel.input_cursor);
								panel.input_cursor -= 1;
							}
						}
						TmKeyCode::Enter => {
							panel.input.insert(panel.input_cursor, "\n");
							panel.input_cursor += 1;
						}
						TmKeyCode::Escape => {
							panel.focused = false;
						}
						_ => {}
					}
				}
				panel_handled = true;
				break;
			}
		}

		if let Some(panel_id) = panel_id_to_submit {
			self.submit_plugin_panel(panel_id);
			return false;
		}
		if panel_handled {
			return false;
		}

		if self.scratch_open && self.scratch_focused {
			// Many terminals send Ctrl+Enter as byte 0x0A (Line Feed = Ctrl+J).
			// Termina parses this as Char('j') with CONTROL modifier.
			// We accept all three variants: Enter, '\n', and 'j' with Ctrl.
			let raw_ctrl_enter = matches!(
				key.code,
				TmKeyCode::Enter | TmKeyCode::Char('\n') | TmKeyCode::Char('j')
			) && key.modifiers.contains(TmModifiers::CONTROL);

			if raw_ctrl_enter {
				return self.with_scratch_context(|ed| ed.do_execute_scratch());
			}
			return self.with_scratch_context(|ed| ed.handle_key_active(key));
		}
		self.handle_key_active(key)
	}

	fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		self.message = None;

		let old_mode = self.mode();
		let key: Key = key.into();
		let in_scratch = self.in_scratch_context;
		if self.scratch_open && self.scratch_focused {
			if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
				if matches!(self.mode(), Mode::Insert) {
					self.input.set_mode(Mode::Normal);
				} else {
					self.do_close_scratch();
				}
				return false;
			}
			let is_enter = matches!(key.code, KeyCode::Special(SpecialKey::Enter))
				|| matches!(key.code, KeyCode::Char('\n'));
			if is_enter && (key.modifiers.ctrl || matches!(self.mode(), Mode::Normal)) {
				return self.do_execute_scratch();
			}
		}

		if in_scratch
			&& matches!(self.mode(), Mode::Insert)
			&& !key.modifiers.alt
			&& !key.modifiers.ctrl
		{
			match key.code {
				KeyCode::Char(c) => {
					self.insert_text(&c.to_string());
					return false;
				}
				KeyCode::Special(SpecialKey::Enter) => {
					self.insert_text("\n");
					return false;
				}
				KeyCode::Special(SpecialKey::Tab) => {
					self.insert_text("\t");
					return false;
				}
				_ => {}
			}
		}

		let result = self.input.handle_key(key);

		match result {
			KeyResult::Action {
				name,
				count,
				extend,
				register,
			} => self.execute_action(name, count, extend, register),
			KeyResult::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => self.execute_action_with_char(name, count, extend, register, char_arg),
			KeyResult::ModeChange(new_mode) => {
				let is_normal = matches!(new_mode, Mode::Normal);
				let leaving_insert = !matches!(new_mode, Mode::Insert);
				if new_mode != old_mode {
					emit_hook(&HookContext::ModeChange {
						old_mode,
						new_mode: new_mode.clone(),
					});
				}
				if is_normal {
					self.message = None;
				}
				if leaving_insert {
					self.insert_undo_active = false;
				}
				false
			}
			KeyResult::InsertChar(c) => {
				self.insert_text(&c.to_string());
				false
			}
			KeyResult::ExecuteCommand(cmd) => self.execute_command_line(&cmd),
			KeyResult::ExecuteSearch { pattern, reverse } => {
				self.input.set_last_search(pattern.clone(), reverse);
				let result = if reverse {
					movement::find_prev(self.doc.slice(..), &pattern, self.cursor)
				} else {
					movement::find_next(self.doc.slice(..), &pattern, self.cursor + 1)
				};
				match result {
					Ok(Some(range)) => {
						self.cursor = range.head;
						self.selection = Selection::single(range.from(), range.to());
						self.show_message(format!("Found: {}", pattern));
					}
					Ok(None) => {
						self.show_message(format!("Pattern not found: {}", pattern));
					}
					Err(e) => {
						self.show_error(format!("Regex error: {}", e));
					}
				}
				false
			}
			KeyResult::SelectRegex { pattern } => {
				self.select_regex(&pattern);
				false
			}
			KeyResult::SplitRegex { pattern } => {
				self.split_regex(&pattern);
				false
			}
			KeyResult::KeepMatching { pattern } => {
				self.keep_matching(&pattern, false);
				false
			}
			KeyResult::KeepNotMatching { pattern } => {
				self.keep_matching(&pattern, true);
				false
			}
			KeyResult::PipeReplace { command } => {
				self.show_error(format!("Pipe (replace) not yet implemented: {}", command));
				false
			}
			KeyResult::PipeIgnore { command } => {
				self.show_error(format!("Pipe (ignore) not yet implemented: {}", command));
				false
			}
			KeyResult::InsertOutput { command } => {
				self.show_error(format!("Insert output not yet implemented: {}", command));
				false
			}
			KeyResult::AppendOutput { command } => {
				self.show_error(format!("Append output not yet implemented: {}", command));
				false
			}
			KeyResult::Consumed => false,
			KeyResult::Unhandled => false,
			KeyResult::Quit => true,
			KeyResult::MouseClick { row, col, extend } => {
				self.handle_mouse_click(row, col, extend);
				false
			}
			KeyResult::MouseDrag { row, col } => {
				self.handle_mouse_drag(row, col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
		}
	}

	pub fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let height = self.window_height.unwrap_or(24);

		// Check terminal panel first (70/30 split when open)
		if self.terminal_open {
			// Terminal takes bottom 30% of main area (before status/message lines)
			let main_area_height = height.saturating_sub(2); // -2 for status and message
			let doc_height = (main_area_height * 70) / 100;
			let term_start = doc_height;
			let term_end = main_area_height;

			if mouse.row >= term_start && mouse.row < term_end {
				// Click is in terminal area - focus it and swallow the event
				if !self.terminal_focused {
					self.terminal_focused = true;
				}
				// Terminal doesn't process mouse events yet, just swallow them
				return false;
			} else if self.terminal_focused {
				// Click outside terminal while focused - unfocus it
				self.terminal_focused = false;
				// Fall through to process click in main editor
			}
		}

		if self.scratch_open {
			let popup_height = 12;
			let popup_y = height.saturating_sub(popup_height + 2); // +2 for status and message
			let popup_end = height.saturating_sub(2);

			// Check if click is inside popup area
			if mouse.row >= popup_y && mouse.row < popup_end {
				// If inside, we handle it in scratchpad

				// First ensure it is focused if it wasn't (e.g. click to focus)
				if !self.scratch_focused {
					self.scratch_focused = true;
				}

				// Adjust mouse coordinates to be relative to the popup
				let mut adj_mouse = mouse;
				adj_mouse.row = mouse.row.saturating_sub(popup_y);

				return self.with_scratch_context(|ed| ed.handle_mouse_active(adj_mouse));
			} else {
				// Click was outside
				// If it's a click (Press), close the popup
				if matches!(mouse.kind, termina::event::MouseEventKind::Down(_)) {
					self.do_close_scratch();
					// Fall through to process click in main editor
				}
			}
		}

		self.handle_mouse_active(mouse)
	}

	pub fn handle_paste(&mut self, content: String) {
		// Route paste to focused terminal first
		if self.terminal_open && self.terminal_focused {
			if let Some(term) = &mut self.terminal {
				let _ = term.write_key(content.as_bytes());
			} else {
				// Terminal is still starting: buffer the paste
				self.terminal_input_buffer
					.extend_from_slice(content.as_bytes());
			}
			return;
		}

		if self.scratch_open && self.scratch_focused {
			self.with_scratch_context(|ed| ed.insert_text(&content));
			return;
		}

		if matches!(self.mode(), Mode::Insert) {
			self.insert_text(&content);
		} else {
			self.show_error("Paste ignored outside insert mode");
		}
	}

	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.window_width = Some(width);
		self.window_height = Some(height);
		emit_hook(&HookContext::WindowResize { width, height });
	}

	pub fn handle_focus_in(&mut self) {
		emit_hook(&HookContext::FocusGained);
	}

	pub fn handle_focus_out(&mut self) {
		emit_hook(&HookContext::FocusLost);
	}

	fn handle_mouse_active(&mut self, mouse: termina::event::MouseEvent) -> bool {
		self.message = None;
		let event: MouseEvent = mouse.into();
		let result = self.input.handle_mouse(event);

		match result {
			KeyResult::MouseClick { row, col, extend } => {
				self.handle_mouse_click(row, col, extend);
				false
			}
			KeyResult::MouseDrag { row, col } => {
				self.handle_mouse_drag(row, col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
			KeyResult::Consumed => false,
			_ => false,
		}
	}

	fn handle_mouse_click(&mut self, screen_row: u16, screen_col: u16, extend: bool) {
		if let Some(doc_pos) = self.screen_to_doc_position(screen_row, screen_col) {
			if extend {
				let anchor = self.selection.primary().anchor;
				self.selection = Selection::single(anchor, doc_pos);
			} else {
				self.selection = Selection::point(doc_pos);
			}
		}
	}

	fn handle_mouse_drag(&mut self, screen_row: u16, screen_col: u16) {
		if let Some(doc_pos) = self.screen_to_doc_position(screen_row, screen_col) {
			let anchor = self.selection.primary().anchor;
			self.selection = Selection::single(anchor, doc_pos);
		}
	}

	fn execute_action(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
	) -> bool {
		use ext::{ActionArgs, ActionContext, find_action};

		let action = match find_action(name) {
			Some(a) => a,
			None => {
				self.show_error(format!("Unknown action: {}", name));
				return false;
			}
		};

		let ctx = ActionContext {
			text: self.doc.slice(..),
			cursor: self.cursor,
			selection: &self.selection,
			count,
			extend,
			register,
			args: ActionArgs::default(),
		};

		let result = (action.handler)(&ctx);
		self.apply_action_result(result, extend)
	}

	fn execute_action_with_char(
		&mut self,
		name: &str,
		count: usize,
		extend: bool,
		register: Option<char>,
		char_arg: char,
	) -> bool {
		use ext::{ActionArgs, ActionContext, find_action};

		let action = match find_action(name) {
			Some(a) => a,
			None => {
				self.show_error(format!("Unknown action: {}", name));
				return false;
			}
		};

		let ctx = ActionContext {
			text: self.doc.slice(..),
			cursor: self.cursor,
			selection: &self.selection,
			count,
			extend,
			register,
			args: ActionArgs {
				char: Some(char_arg),
				string: None,
			},
		};

		let result = (action.handler)(&ctx);
		self.apply_action_result(result, extend)
	}

	fn apply_action_result(&mut self, result: ext::ActionResult, extend: bool) -> bool {
		let mut ctx = ext::EditorContext::new(self);
		ext::dispatch_result(&result, &mut ctx, extend)
	}
}

impl ext::EditorOps for Editor {
	fn path(&self) -> Option<&std::path::Path> {
		self.path.as_deref()
	}

	fn text(&self) -> tome_core::RopeSlice<'_> {
		self.doc.slice(..)
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.selection
	}

	fn message(&mut self, msg: &str) {
		self.show_message(msg);
	}

	fn error(&mut self, msg: &str) {
		self.show_error(msg);
	}

	fn save(&mut self) -> Result<(), ext::CommandError> {
		Editor::save(self).map_err(|e| ext::CommandError::Io(e.to_string()))
	}

	fn save_as(&mut self, path: std::path::PathBuf) -> Result<(), ext::CommandError> {
		Editor::save_as(self, path).map_err(|e| ext::CommandError::Io(e.to_string()))
	}

	fn insert_text(&mut self, text: &str) {
		Editor::insert_text(self, text);
	}

	fn delete_selection(&mut self) {
		if !self.selection.primary().is_empty() {
			self.save_undo_state();
			let tx = Transaction::delete(self.doc.slice(..), &self.selection);
			self.selection = tx.map_selection(&self.selection);
			tx.apply(&mut self.doc);
			self.modified = true;
		}
	}

	fn set_modified(&mut self, modified: bool) {
		self.modified = modified;
	}

	fn is_modified(&self) -> bool {
		self.modified
	}

	fn on_permission_decision(&mut self, request_id: u64, option_id: &str) -> Result<(), String> {
		let pos = self
			.pending_permissions
			.iter()
			.position(|p| p.request_id == request_id)
			.ok_or_else(|| format!("No pending permission request with ID {}", request_id))?;

		let pending = self.pending_permissions.remove(pos);
		let plugin_idx = pending.plugin_idx;

		if let Some(plugin) = self.plugins.plugins.get(plugin_idx)
			&& let Some(on_decision) = plugin.guest.on_permission_decision
		{
			use crate::plugins::manager::PluginContextGuard;
			let ed_ptr = self as *mut Editor;
			let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
			let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, plugin_idx) };
			let option_tome = tome_cabi_types::TomeStr {
				ptr: option_id.as_ptr(),
				len: option_id.len(),
			};
			on_decision(request_id, option_tome);
			return Ok(());
		}

		Err(format!(
			"Plugin {} does not support permission decisions",
			plugin_idx
		))
	}

	fn set_theme(&mut self, theme_name: &str) -> Result<(), String> {
		if let Some(theme) = crate::theme::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = crate::theme::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(err)
		}
	}
}
