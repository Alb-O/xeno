//! Terminal emulator as a SplitBuffer.
//!
//! This module provides `TerminalBuffer`, which wraps a PTY-backed terminal
//! emulator and exposes it via the `SplitBuffer` trait for use in split panels.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tome_manifest::{
	SplitAttrs, SplitBuffer, SplitCell, SplitColor, SplitCursor, SplitCursorStyle,
	SplitDockPreference, SplitEventResult, SplitKey, SplitKeyCode, SplitModifiers, SplitSize,
};
use tome_tui::widgets::terminal::vt100::{self, Parser};

use crate::terminal_ipc::{TerminalIpcEnv, fish_init_command};

/// Error type for terminal operations.
#[derive(thiserror::Error, Debug)]
pub enum TerminalError {
	#[error("PTY error: {0}")]
	Pty(String),
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Spawn error: {0}")]
	Spawn(String),
}

/// Internal terminal state wrapping a PTY and parser.
struct TerminalState {
	parser: Parser,
	pty_master: Box<dyn MasterPty + Send>,
	pty_writer: Box<dyn Write + Send>,
	receiver: Receiver<Vec<u8>>,
	child: Box<dyn portable_pty::Child + Send>,
	fish_init: Option<FishInitState>,
	/// Cursor shape set via DECSCUSR (ESC [ Ps SP q)
	cursor_shape: SplitCursorStyle,
}

struct FishInitState {
	pid: u32,
	tome_bin: String,
	tome_socket: String,
	last_check: Instant,
	attempts: u32,
}

const FISH_INIT_CHECK_INTERVAL: Duration = Duration::from_millis(50);
const FISH_INIT_MAX_ATTEMPTS: u32 = 100;

impl TerminalState {
	fn new(cols: u16, rows: u16, env_vars: Vec<(String, String)>) -> Result<Self, TerminalError> {
		let pty_system = NativePtySystem::default();
		let pair = pty_system
			.openpty(PtySize {
				rows,
				cols,
				pixel_width: 0,
				pixel_height: 0,
			})
			.map_err(|e| TerminalError::Pty(e.to_string()))?;

		let tome_bin = env_vars
			.iter()
			.find(|(key, _)| key == "TOME_BIN")
			.map(|(_, value)| value.clone());
		let tome_socket = env_vars
			.iter()
			.find(|(key, _)| key == "TOME_SOCKET")
			.map(|(_, value)| value.clone());

		let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
		let shell_name = Path::new(&shell)
			.file_name()
			.and_then(|name| name.to_str())
			.unwrap_or(&shell)
			.to_string();
		let mut cmd = CommandBuilder::new(&shell);
		for (key, value) in env_vars {
			cmd.env(key, value);
		}
		apply_shell_path_injection(
			&mut cmd,
			&shell,
			tome_bin.as_deref(),
			tome_socket.as_deref(),
		);

		let child = pair
			.slave
			.spawn_command(cmd)
			.map_err(|e| TerminalError::Spawn(e.to_string()))?;

		let mut reader = pair
			.master
			.try_clone_reader()
			.map_err(|e| TerminalError::Pty(e.to_string()))?;
		let mut writer = pair
			.master
			.take_writer()
			.map_err(|e| TerminalError::Pty(e.to_string()))?;

		let mut fish_init = None;
		if let (Some(tome_bin), Some(tome_socket)) = (tome_bin.clone(), tome_socket.clone()) {
			if shell_name == "fish" {
				inject_shell_init("fish", &mut writer, &tome_bin, &tome_socket);
			} else if let Some(pid) = child.process_id() {
				if is_fish_process(pid) {
					inject_shell_init("fish", &mut writer, &tome_bin, &tome_socket);
				} else {
					fish_init = Some(FishInitState {
						pid,
						tome_bin,
						tome_socket,
						last_check: Instant::now(),
						attempts: 0,
					});
				}
			}
		}

		let master = pair.master;

		let (tx, rx) = channel();

		thread::spawn(move || {
			let mut buf = [0u8; 4096];
			loop {
				match reader.read(&mut buf) {
					Ok(n) if n > 0 => {
						if tx.send(buf[..n].to_vec()).is_err() {
							break;
						}
					}
					_ => break,
				}
			}
		});

		Ok(Self {
			parser: Parser::new(rows, cols, 0),
			pty_master: master,
			pty_writer: writer,
			receiver: rx,
			child,
			fish_init,
			cursor_shape: SplitCursorStyle::Default,
		})
	}

	fn screen(&self) -> &vt100::Screen {
		self.parser.screen()
	}

	fn update(&mut self) {
		self.poll_fish_init();
		loop {
			match self.receiver.try_recv() {
				Ok(bytes) => {
					// Handle escape sequences that vt100 doesn't process
					self.handle_escape_sequences(&bytes);
					self.parser.process(&bytes);
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => break,
			}
		}
	}

	/// Handles escape sequences that vt100 doesn't process (DA1, DSR, DECSCUSR).
	fn handle_escape_sequences(&mut self, bytes: &[u8]) {
		// DA1: ESC[c or ESC[0c → respond with VT102 identifier
		if bytes.windows(3).any(|w| w == b"\x1b[c") || bytes.windows(4).any(|w| w == b"\x1b[0c") {
			let _ = self.pty_writer.write_all(b"\x1b[?6c");
		}
		// DSR: ESC[6n → respond with cursor position
		if bytes.windows(4).any(|w| w == b"\x1b[6n") {
			let (row, col) = self.parser.screen().cursor_position();
			let response = format!("\x1b[{};{}R", row + 1, col + 1);
			let _ = self.pty_writer.write_all(response.as_bytes());
		}
		// DECSCUSR: track cursor shape
		self.cursor_shape = parse_decscusr(bytes).unwrap_or(self.cursor_shape);
	}

	fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
		self.parser.set_size(rows, cols);
		self.pty_master
			.resize(PtySize {
				rows,
				cols,
				pixel_width: 0,
				pixel_height: 0,
			})
			.map_err(|e| TerminalError::Pty(e.to_string()))
	}

	fn write_key(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
		self.pty_writer.write_all(bytes).map_err(TerminalError::Io)
	}

	fn is_alive(&mut self) -> bool {
		match self.child.try_wait() {
			Ok(Some(_)) => false,
			Ok(None) => true,
			Err(_) => false,
		}
	}

	fn poll_fish_init(&mut self) {
		let Some(state) = self.fish_init.as_mut() else {
			return;
		};

		if state.last_check.elapsed() < FISH_INIT_CHECK_INTERVAL {
			return;
		}

		state.last_check = Instant::now();
		state.attempts += 1;

		if is_fish_process(state.pid) {
			inject_shell_init(
				"fish",
				&mut self.pty_writer,
				&state.tome_bin,
				&state.tome_socket,
			);
			self.fish_init = None;
			return;
		}

		if state.attempts >= FISH_INIT_MAX_ATTEMPTS {
			self.fish_init = None;
		}
	}
}

fn apply_shell_path_injection(
	cmd: &mut CommandBuilder,
	shell: &str,
	tome_bin: Option<&str>,
	tome_socket: Option<&str>,
) {
	let Some(tome_bin) = tome_bin else {
		return;
	};
	let Some(socket) = tome_socket else {
		return;
	};

	let shell_name = Path::new(shell)
		.file_name()
		.and_then(|name| name.to_str())
		.unwrap_or(shell);

	let bin_path = Path::new(tome_bin);
	let socket_path = Path::new(socket);

	match shell_name {
		"fish" => {
			let init = fish_init_command(bin_path, socket_path);
			cmd.arg("-i");
			cmd.arg("--init-command");
			cmd.arg(init);
		}
		"zsh" => {
			cmd.arg("-f");
		}
		// nushell's line editor (reedline) doesn't work in the embedded terminal,
		// so we don't add any special handling for it
		"bash" | "nu" => {}
		_ => {}
	}
}

fn inject_shell_init(shell_name: &str, writer: &mut dyn Write, tome_bin: &str, tome_socket: &str) {
	let init = match shell_name {
		"fish" => format!(
			"set -gx TOME_BIN {tome_bin}; set -gx TOME_SOCKET {tome_socket}; \
set -gx PATH {tome_bin} $PATH; \
function fish_command_not_found; set -l cmd $argv[1]; \
if string match -q ':*' -- $cmd; set -l target \"$TOME_BIN/$cmd\"; \
if test -x \"$target\"; \"$target\" $argv[2..-1]; return $status; end; end; \
if functions -q __fish_command_not_found_handler; __fish_command_not_found_handler $argv; end; \
return 127; end\n"
		),
		"bash" => format!(
			"export TOME_BIN=\"{tome_bin}\"; export TOME_SOCKET=\"{tome_socket}\"; \
export PATH=\"$TOME_BIN:$PATH\"; \
command_not_found_handle() {{ local cmd=\"$1\"; shift; \
if [[ \"$cmd\" == :* ]] && [[ -x \"$TOME_BIN/$cmd\" ]]; then \
\"$TOME_BIN/$cmd\" \"$@\"; return $?; fi; \
echo \"bash: $cmd: command not found\" >&2; return 127; }}\n"
		),
		"zsh" => format!(
			"export TOME_BIN=\"{tome_bin}\"; export TOME_SOCKET=\"{tome_socket}\"; \
export PATH=\"$TOME_BIN:$PATH\"; \
command_not_found_handler() {{ local cmd=\"$1\"; shift; \
if [[ \"$cmd\" == :* ]] && [[ -x \"$TOME_BIN/$cmd\" ]]; then \
\"$TOME_BIN/$cmd\" \"$@\"; return $?; fi; \
echo \"zsh: command not found: $cmd\" >&2; return 127; }}\n"
		),
		// nushell's line editor (reedline) doesn't work in the embedded terminal
		_ => return,
	};

	let _ = writer.write_all(init.as_bytes());
}

#[cfg(target_os = "linux")]
fn read_proc_comm(pid: u32) -> Option<String> {
	let contents = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
	let trimmed = contents.trim();
	if trimmed.is_empty() {
		None
	} else {
		Some(trimmed.to_string())
	}
}

fn is_fish_process(pid: u32) -> bool {
	#[cfg(target_os = "linux")]
	{
		read_proc_comm(pid).as_deref() == Some("fish")
	}

	#[cfg(not(target_os = "linux"))]
	{
		let _ = pid;
		false
	}
}

/// A terminal emulator that implements [`SplitBuffer`].
///
/// Provides an embedded terminal in a split panel:
/// - Spawns a shell process (from `$SHELL` or defaults to `sh`)
/// - Handles input/output via a PTY
/// - Renders terminal cells for the UI layer
///
/// When created with [`Self::with_ipc`], shell wrappers for editor commands
/// (e.g., `:write`, `:quit`) are available in `$PATH`.
pub struct TerminalBuffer {
	terminal: Option<TerminalState>,
	prewarm: Option<Receiver<Result<TerminalState, TerminalError>>>,
	input_buffer: Vec<u8>,
	current_size: SplitSize,
	ipc_env: Option<Arc<TerminalIpcEnv>>,
}

impl Default for TerminalBuffer {
	fn default() -> Self {
		Self::new()
	}
}

impl TerminalBuffer {
	/// Creates a new terminal buffer.
	///
	/// The terminal is not spawned immediately; prewarming begins on [`SplitBuffer::on_open`].
	pub fn new() -> Self {
		Self {
			terminal: None,
			prewarm: None,
			input_buffer: Vec::new(),
			current_size: SplitSize::new(80, 24),
			ipc_env: None,
		}
	}

	/// Creates a terminal buffer with IPC integration for editor commands.
	pub fn with_ipc(ipc_env: Arc<TerminalIpcEnv>) -> Self {
		Self {
			terminal: None,
			prewarm: None,
			input_buffer: Vec::new(),
			current_size: SplitSize::new(80, 24),
			ipc_env: Some(ipc_env),
		}
	}

	/// Returns a reference to the internal vt100 screen, if available.
	///
	/// This is useful for extensions that need direct access to terminal state.
	pub fn screen(&self) -> Option<&vt100::Screen> {
		self.terminal.as_ref().map(|t| t.screen())
	}

	fn start_prewarm(&mut self) {
		if self.terminal.is_some() || self.prewarm.is_some() {
			return;
		}
		let size = self.current_size;
		let env_vars = self
			.ipc_env
			.as_ref()
			.map(|env| env.env_vars())
			.unwrap_or_default();

		log::debug!(
			"terminal prewarm: ipc_env={}, env_vars_count={}",
			self.ipc_env.is_some(),
			env_vars.len()
		);

		let (tx, rx) = channel();
		self.prewarm = Some(rx);
		thread::spawn(move || {
			let _ = tx.send(TerminalState::new(size.width, size.height, env_vars));
		});
	}

	fn poll_prewarm(&mut self) -> bool {
		let Some(rx) = self.prewarm.as_ref() else {
			return false;
		};
		match rx.try_recv() {
			Ok(Ok(mut term)) => {
				if !self.input_buffer.is_empty() {
					let _ = term.write_key(&self.input_buffer);
					self.input_buffer.clear();
				}
				self.terminal = Some(term);
				self.prewarm = None;
				true
			}
			Ok(Err(_e)) => {
				self.prewarm = None;
				true
			}
			Err(TryRecvError::Empty) => false,
			Err(TryRecvError::Disconnected) => {
				self.prewarm = None;
				true
			}
		}
	}

	fn key_to_bytes(key: &SplitKey) -> Vec<u8> {
		match key.code {
			SplitKeyCode::Char(c) => {
				if key.modifiers.contains(SplitModifiers::CTRL) {
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
			SplitKeyCode::Enter => vec![b'\r'],
			SplitKeyCode::Backspace => vec![0x7f],
			SplitKeyCode::Tab => vec![b'\t'],
			SplitKeyCode::Up => b"\x1b[A".to_vec(),
			SplitKeyCode::Down => b"\x1b[B".to_vec(),
			SplitKeyCode::Right => b"\x1b[C".to_vec(),
			SplitKeyCode::Left => b"\x1b[D".to_vec(),
			SplitKeyCode::Home => b"\x1b[H".to_vec(),
			SplitKeyCode::End => b"\x1b[F".to_vec(),
			SplitKeyCode::PageUp => b"\x1b[5~".to_vec(),
			SplitKeyCode::PageDown => b"\x1b[6~".to_vec(),
			SplitKeyCode::Delete => b"\x1b[3~".to_vec(),
			SplitKeyCode::Insert => b"\x1b[2~".to_vec(),
			SplitKeyCode::F(n) => match n {
				1 => b"\x1bOP".to_vec(),
				2 => b"\x1bOQ".to_vec(),
				3 => b"\x1bOR".to_vec(),
				4 => b"\x1bOS".to_vec(),
				5 => b"\x1b[15~".to_vec(),
				6 => b"\x1b[17~".to_vec(),
				7 => b"\x1b[18~".to_vec(),
				8 => b"\x1b[19~".to_vec(),
				9 => b"\x1b[20~".to_vec(),
				10 => b"\x1b[21~".to_vec(),
				11 => b"\x1b[23~".to_vec(),
				12 => b"\x1b[24~".to_vec(),
				_ => vec![],
			},
			SplitKeyCode::Escape => vec![0x1b],
		}
	}
}

impl SplitBuffer for TerminalBuffer {
	fn id(&self) -> &str {
		"terminal"
	}

	fn on_open(&mut self) {
		self.start_prewarm();
	}

	fn on_close(&mut self) {
		self.input_buffer.clear();
	}

	fn resize(&mut self, size: SplitSize) {
		self.current_size = size;
		if let Some(term) = &mut self.terminal {
			let _ = term.resize(size.width, size.height);
		}
	}

	fn tick(&mut self, _delta: Duration) -> SplitEventResult {
		let mut changed = self.poll_prewarm();

		let mut terminal_exited = false;
		if let Some(term) = &mut self.terminal {
			term.update();
			if !term.is_alive() {
				terminal_exited = true;
			}
			changed = true;
		}

		if terminal_exited {
			self.terminal = None;
			self.prewarm = None;
			self.input_buffer.clear();
			self.start_prewarm();
			return SplitEventResult::consumed().with_close();
		}

		if changed {
			SplitEventResult {
				needs_redraw: true,
				..Default::default()
			}
		} else {
			SplitEventResult::ignored()
		}
	}

	fn handle_key(&mut self, key: SplitKey) -> SplitEventResult {
		// Escape releases focus (handled by SplitBufferPanel wrapper)
		if matches!(key.code, SplitKeyCode::Escape) {
			return SplitEventResult::consumed().with_release_focus();
		}

		let bytes = Self::key_to_bytes(&key);
		if bytes.is_empty() {
			return SplitEventResult::ignored();
		}

		if let Some(term) = &mut self.terminal {
			let _ = term.write_key(&bytes);
		} else {
			self.input_buffer.extend_from_slice(&bytes);
		}

		SplitEventResult::consumed()
	}

	fn handle_paste(&mut self, text: &str) -> SplitEventResult {
		if let Some(term) = &mut self.terminal {
			let _ = term.write_key(text.as_bytes());
		} else {
			self.input_buffer.extend_from_slice(text.as_bytes());
		}
		SplitEventResult::consumed()
	}

	fn size(&self) -> SplitSize {
		self.current_size
	}

	fn cursor(&self) -> Option<SplitCursor> {
		let term = self.terminal.as_ref()?;
		let screen = term.screen();
		if screen.hide_cursor() {
			return None;
		}
		let (row, col) = screen.cursor_position();
		Some(SplitCursor {
			row,
			col,
			style: term.cursor_shape,
		})
	}

	fn for_each_cell<F>(&self, mut f: F)
	where
		F: FnMut(u16, u16, &SplitCell),
	{
		let Some(term) = &self.terminal else {
			return;
		};
		let screen = term.screen();
		let (rows, cols) = screen.size();

		for row in 0..rows {
			for col in 0..cols {
				let Some(cell) = screen.cell(row, col) else {
					continue;
				};

				if cell.is_wide_continuation() {
					let split_cell = SplitCell {
						wide_continuation: true,
						..Default::default()
					};
					f(row, col, &split_cell);
					continue;
				}

				let fg = map_vt_color(cell.fgcolor());
				let bg = map_vt_color(cell.bgcolor());

				let mut attrs = SplitAttrs::NONE;
				if cell.bold() {
					attrs = attrs.union(SplitAttrs::BOLD);
				}
				if cell.italic() {
					attrs = attrs.union(SplitAttrs::ITALIC);
				}
				if cell.underline() {
					attrs = attrs.union(SplitAttrs::UNDERLINE);
				}
				if cell.inverse() {
					attrs = attrs.union(SplitAttrs::INVERSE);
				}

				let symbol = if cell.has_contents() {
					cell.contents()
				} else {
					" ".to_string()
				};

				let split_cell = SplitCell {
					symbol,
					fg,
					bg,
					attrs,
					wide_continuation: false,
				};
				f(row, col, &split_cell);
			}
		}
	}
}

fn map_vt_color(color: vt100::Color) -> Option<SplitColor> {
	match color {
		vt100::Color::Default => None,
		vt100::Color::Idx(i) => Some(SplitColor::Indexed(i)),
		vt100::Color::Rgb(r, g, b) => Some(SplitColor::Rgb(r, g, b)),
	}
}

/// Parses DECSCUSR (Set Cursor Style): `ESC [ Ps SP q`
fn parse_decscusr(bytes: &[u8]) -> Option<SplitCursorStyle> {
	let mut i = 0;
	while i + 4 <= bytes.len() {
		if bytes[i] == 0x1b && bytes[i + 1] == b'[' {
			let start = i + 2;
			let mut end = start;
			while end < bytes.len() && bytes[end].is_ascii_digit() {
				end += 1;
			}
			if end + 2 <= bytes.len() && bytes[end] == b' ' && bytes[end + 1] == b'q' {
				let ps = std::str::from_utf8(&bytes[start..end])
					.ok()
					.and_then(|s| s.parse::<u8>().ok())
					.unwrap_or(0);
				return Some(match ps {
					0 | 1 => SplitCursorStyle::BlinkingBlock,
					2 => SplitCursorStyle::Block,
					3 => SplitCursorStyle::BlinkingUnderline,
					4 => SplitCursorStyle::Underline,
					5 => SplitCursorStyle::BlinkingBar,
					6 => SplitCursorStyle::Bar,
					_ => SplitCursorStyle::Default,
				});
			}
		}
		i += 1;
	}
	None
}

/// Dock preference for terminal buffer.
impl TerminalBuffer {
	/// Returns the preferred dock position for terminals.
	pub fn dock_preference() -> SplitDockPreference {
		SplitDockPreference::Bottom
	}
}
