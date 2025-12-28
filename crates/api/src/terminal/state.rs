//! Terminal PTY state management.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use evildoer_manifest::SplitCursorStyle;
use evildoer_tui::widgets::terminal::vt100::{self, Parser};

use super::escape::parse_decscusr;
use crate::terminal_ipc::fish_init_command;

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
pub(super) struct TerminalState {
	pub parser: Parser,
	pub pty_master: Box<dyn MasterPty + Send>,
	pub pty_writer: Box<dyn Write + Send>,
	pub receiver: Receiver<Vec<u8>>,
	pub child: Box<dyn portable_pty::Child + Send>,
	pub fish_init: Option<FishInitState>,
	/// Cursor shape set via DECSCUSR (ESC [ Ps SP q)
	pub cursor_shape: SplitCursorStyle,
}

pub(super) struct FishInitState {
	pub pid: u32,
	pub evildoer_bin: String,
	pub evildoer_socket: String,
	pub last_check: Instant,
	pub attempts: u32,
}

const FISH_INIT_CHECK_INTERVAL: Duration = Duration::from_millis(50);
const FISH_INIT_MAX_ATTEMPTS: u32 = 100;

impl TerminalState {
	pub fn new(
		cols: u16,
		rows: u16,
		env_vars: Vec<(String, String)>,
	) -> Result<Self, TerminalError> {
		let pty_system = NativePtySystem::default();
		let pair = pty_system
			.openpty(PtySize {
				rows,
				cols,
				pixel_width: 0,
				pixel_height: 0,
			})
			.map_err(|e| TerminalError::Pty(e.to_string()))?;

		let evildoer_bin = env_vars
			.iter()
			.find(|(key, _)| key == "EVILDOER_BIN")
			.map(|(_, value)| value.clone());
		let evildoer_socket = env_vars
			.iter()
			.find(|(key, _)| key == "EVILDOER_SOCKET")
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
			evildoer_bin.as_deref(),
			evildoer_socket.as_deref(),
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
		if let (Some(evildoer_bin), Some(evildoer_socket)) = (evildoer_bin.clone(), evildoer_socket.clone()) {
			if shell_name == "fish" {
				inject_shell_init("fish", &mut writer, &evildoer_bin, &evildoer_socket);
			} else if let Some(pid) = child.process_id() {
				if is_fish_process(pid) {
					inject_shell_init("fish", &mut writer, &evildoer_bin, &evildoer_socket);
				} else {
					fish_init = Some(FishInitState {
						pid,
						evildoer_bin,
						evildoer_socket,
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

	pub fn screen(&self) -> &vt100::Screen {
		self.parser.screen()
	}

	pub fn update(&mut self) {
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
		// DA1: ESC[c or ESC[0c -> respond with VT102 identifier
		if bytes.windows(3).any(|w| w == b"\x1b[c") || bytes.windows(4).any(|w| w == b"\x1b[0c") {
			let _ = self.pty_writer.write_all(b"\x1b[?6c");
		}
		// DSR: ESC[6n -> respond with cursor position
		if bytes.windows(4).any(|w| w == b"\x1b[6n") {
			let (row, col) = self.parser.screen().cursor_position();
			let response = format!("\x1b[{};{}R", row + 1, col + 1);
			let _ = self.pty_writer.write_all(response.as_bytes());
		}
		// DECSCUSR: track cursor shape
		self.cursor_shape = parse_decscusr(bytes).unwrap_or(self.cursor_shape);
	}

	pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
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

	pub fn write_key(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
		self.pty_writer.write_all(bytes).map_err(TerminalError::Io)
	}

	pub fn is_alive(&mut self) -> bool {
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
				&state.evildoer_bin,
				&state.evildoer_socket,
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
	evildoer_bin: Option<&str>,
	evildoer_socket: Option<&str>,
) {
	let Some(evildoer_bin) = evildoer_bin else {
		return;
	};
	let Some(socket) = evildoer_socket else {
		return;
	};

	let shell_name = Path::new(shell)
		.file_name()
		.and_then(|name| name.to_str())
		.unwrap_or(shell);

	let bin_path = Path::new(evildoer_bin);
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

fn inject_shell_init(shell_name: &str, writer: &mut dyn Write, evildoer_bin: &str, evildoer_socket: &str) {
	let init = match shell_name {
		"fish" => format!(
			"set -gx EVILDOER_BIN {evildoer_bin}; set -gx EVILDOER_SOCKET {evildoer_socket}; \
set -gx PATH {evildoer_bin} $PATH; \
function fish_command_not_found; set -l cmd $argv[1]; \
if string match -q ':*' -- $cmd; set -l target \"$EVILDOER_BIN/$cmd\"; \
if test -x \"$target\"; \"$target\" $argv[2..-1]; return $status; end; end; \
if functions -q __fish_command_not_found_handler; __fish_command_not_found_handler $argv; end; \
return 127; end\n"
		),
		"bash" => format!(
			"export EVILDOER_BIN=\"{evildoer_bin}\"; export EVILDOER_SOCKET=\"{evildoer_socket}\"; \
export PATH=\"$EVILDOER_BIN:$PATH\"; \
command_not_found_handle() {{ local cmd=\"$1\"; shift; \
if [[ \"$cmd\" == :* ]] && [[ -x \"$EVILDOER_BIN/$cmd\" ]]; then \
\"$EVILDOER_BIN/$cmd\" \"$@\"; return $?; fi; \
echo \"bash: $cmd: command not found\" >&2; return 127; }}\n"
		),
		"zsh" => format!(
			"export EVILDOER_BIN=\"{evildoer_bin}\"; export EVILDOER_SOCKET=\"{evildoer_socket}\"; \
export PATH=\"$EVILDOER_BIN:$PATH\"; \
command_not_found_handler() {{ local cmd=\"$1\"; shift; \
if [[ \"$cmd\" == :* ]] && [[ -x \"$EVILDOER_BIN/$cmd\" ]]; then \
\"$EVILDOER_BIN/$cmd\" \"$@\"; return $?; fi; \
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
