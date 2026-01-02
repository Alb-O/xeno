//! Terminal PTY state management.

use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread;

use evildoer_registry::panels::SplitCursorStyle;
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use vt100::{self, Parser};

use super::escape::parse_decscusr;

/// Error type for terminal operations.
#[derive(thiserror::Error, Debug)]
pub enum TerminalError {
	/// Error from the pseudoterminal subsystem.
	#[error("PTY error: {0}")]
	Pty(String),
	/// Standard I/O error.
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	/// Error spawning the shell process.
	#[error("Spawn error: {0}")]
	Spawn(String),
}

/// Internal terminal state wrapping a PTY and parser.
pub(super) struct TerminalState {
	/// VT100 terminal emulator parser.
	pub parser: Parser,
	/// Master end of the pseudoterminal for resize operations.
	pub pty_master: Box<dyn MasterPty + Send>,
	/// Writer for sending input to the terminal.
	pub pty_writer: Box<dyn Write + Send>,
	/// Receiver for output from the terminal's reader thread.
	pub receiver: Receiver<Vec<u8>>,
	/// Handle to the spawned shell process.
	pub child: Box<dyn portable_pty::Child + Send>,
	/// Cursor shape set via DECSCUSR (ESC [ Ps SP q).
	pub cursor_shape: SplitCursorStyle,
}

impl TerminalState {
	/// Creates a new terminal state with the given dimensions.
	///
	/// Spawns a shell process (from `$SHELL` or `sh`) in a new PTY.
	pub fn new(cols: u16, rows: u16) -> Result<Self, TerminalError> {
		let pty_system = NativePtySystem::default();
		let pair = pty_system
			.openpty(PtySize {
				rows,
				cols,
				pixel_width: 0,
				pixel_height: 0,
			})
			.map_err(|e| TerminalError::Pty(e.to_string()))?;

		let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
		let cmd = CommandBuilder::new(&shell);

		let child = pair
			.slave
			.spawn_command(cmd)
			.map_err(|e| TerminalError::Spawn(e.to_string()))?;

		let mut reader = pair
			.master
			.try_clone_reader()
			.map_err(|e| TerminalError::Pty(e.to_string()))?;
		let writer = pair
			.master
			.take_writer()
			.map_err(|e| TerminalError::Pty(e.to_string()))?;

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
			cursor_shape: SplitCursorStyle::Default,
		})
	}

	/// Returns a reference to the terminal screen state.
	pub fn screen(&self) -> &vt100::Screen {
		self.parser.screen()
	}

	/// Processes any pending output from the PTY reader thread.
	pub fn update(&mut self) {
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

	/// Resizes the terminal to the given dimensions.
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

	/// Writes key input bytes to the terminal.
	pub fn write_key(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
		self.pty_writer.write_all(bytes).map_err(TerminalError::Io)
	}

	/// Returns whether the shell process is still running.
	pub fn is_alive(&mut self) -> bool {
		match self.child.try_wait() {
			Ok(Some(_)) => false,
			Ok(None) => true,
			Err(_) => false,
		}
	}
}
