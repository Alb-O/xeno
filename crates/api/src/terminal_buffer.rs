//! Terminal emulator as a SplitBuffer.
//!
//! This module provides `TerminalBuffer`, which wraps a PTY-backed terminal
//! emulator and exposes it via the `SplitBuffer` trait for use in split panels.

use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::Duration;

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tome_manifest::{
	SplitAttrs, SplitBuffer, SplitCell, SplitColor, SplitCursor, SplitCursorStyle,
	SplitDockPreference, SplitEventResult, SplitKey, SplitKeyCode, SplitModifiers, SplitSize,
};
use tui_term::vt100::{self, Parser};

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
}

impl TerminalState {
	fn new(cols: u16, rows: u16) -> Result<Self, TerminalError> {
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
		let cmd = CommandBuilder::new(shell);

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
		})
	}

	fn screen(&self) -> &vt100::Screen {
		self.parser.screen()
	}

	fn update(&mut self) {
		loop {
			match self.receiver.try_recv() {
				Ok(bytes) => {
					// Handle DA1 query from shells like fish
					let da1_query = b"\x1b[c";
					let da1_query_0 = b"\x1b[0c";

					if bytes.windows(da1_query.len()).any(|w| w == da1_query)
						|| bytes.windows(da1_query_0.len()).any(|w| w == da1_query_0)
					{
						let _ = self.pty_writer.write_all(b"\x1b[?6c");
					}

					self.parser.process(&bytes);
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => break,
			}
		}
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
}

/// A terminal emulator that implements `SplitBuffer`.
///
/// This provides an embedded terminal in a split panel. The terminal:
/// - Spawns a shell process (from `$SHELL` or defaults to `sh`)
/// - Handles input/output via a PTY
/// - Renders terminal cells for the UI layer
///
/// # Usage
///
/// Terminals are opened via `Editor::split_horizontal_terminal()` or
/// `Editor::split_vertical_terminal()`, typically triggered by the
/// `Ctrl+w t` or `Ctrl+w T` keybindings.
pub struct TerminalBuffer {
	terminal: Option<TerminalState>,
	prewarm: Option<Receiver<Result<TerminalState, TerminalError>>>,
	input_buffer: Vec<u8>,
	current_size: SplitSize,
}

impl Default for TerminalBuffer {
	fn default() -> Self {
		Self::new()
	}
}

impl TerminalBuffer {
	/// Creates a new terminal buffer.
	///
	/// The terminal is not spawned immediately; instead, it begins prewarming
	/// a shell in the background so that opening is instant.
	pub fn new() -> Self {
		Self {
			terminal: None,
			prewarm: None,
			input_buffer: Vec::new(),
			current_size: SplitSize::new(80, 24),
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
		let (tx, rx) = channel();
		self.prewarm = Some(rx);
		thread::spawn(move || {
			let _ = tx.send(TerminalState::new(size.width, size.height));
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
		let screen = self.terminal.as_ref()?.screen();
		if screen.hide_cursor() {
			return None;
		}
		let (row, col) = screen.cursor_position();
		Some(SplitCursor {
			row,
			col,
			style: SplitCursorStyle::Default,
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

/// Dock preference for terminal buffer.
impl TerminalBuffer {
	/// Returns the preferred dock position for terminals.
	pub fn dock_preference() -> SplitDockPreference {
		SplitDockPreference::Bottom
	}
}
