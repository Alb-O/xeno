//! Terminal emulator as a SplitBuffer.
//!
//! This module provides `TerminalBuffer`, which wraps a PTY-backed terminal
//! emulator and exposes it via the `SplitBuffer` trait for use in split panels.

mod color;
mod escape;
mod key;
mod selection;
mod state;

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::Duration;

use evildoer_registry::panels::{
	SplitAttrs, SplitBuffer, SplitCell, SplitCursor, SplitEventResult, SplitKey, SplitKeyCode,
	SplitMouse, SplitMouseAction, SplitMouseButton, SplitSize,
};

use self::color::map_vt_color;
use self::key::key_to_bytes;
use self::selection::TerminalSelection;
pub use self::state::TerminalError as Error;
use self::state::{TerminalError, TerminalState};

/// A terminal emulator that implements [`SplitBuffer`].
///
/// Provides an embedded terminal in a split panel:
/// - Spawns a shell process (from `$SHELL` or defaults to `sh`)
/// - Handles input/output via a PTY
/// - Renders terminal cells for the UI layer
pub struct TerminalBuffer {
	/// Active terminal state, if spawned.
	terminal: Option<TerminalState>,
	/// Receiver for prewarm spawn result.
	prewarm: Option<Receiver<Result<TerminalState, TerminalError>>>,
	/// Buffered input while terminal is spawning.
	input_buffer: Vec<u8>,
	/// Current terminal dimensions.
	current_size: SplitSize,
	/// Active text selection, if any.
	selection: Option<TerminalSelection>,
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
			selection: None,
		}
	}

	/// Returns a reference to the internal vt100 screen, if available.
	///
	/// This is useful for extensions that need direct access to terminal state.
	pub fn screen(&self) -> Option<&vt100::Screen> {
		self.terminal.as_ref().map(|t| t.screen())
	}

	/// Starts prewarming the terminal in a background thread.
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

	/// Polls the prewarm channel for completion, returns true if state changed.
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

		let bytes = key_to_bytes(&key);
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

	fn handle_mouse(&mut self, mouse: SplitMouse) -> SplitEventResult {
		use SplitMouseAction::*;
		use SplitMouseButton::*;

		let (row, col) = (mouse.position.y, mouse.position.x);

		match mouse.action {
			Press(Left) => {
				self.selection = Some(TerminalSelection {
					anchor_row: row,
					anchor_col: col,
					cursor_row: row,
					cursor_col: col,
				});
				SplitEventResult::consumed()
			}
			Drag(Left) => {
				if let Some(sel) = &mut self.selection {
					sel.cursor_row = row;
					sel.cursor_col = col;
				}
				SplitEventResult::consumed()
			}
			Release(Left) => SplitEventResult::consumed(),
			Press(Right | Middle) => {
				self.selection = None;
				SplitEventResult::consumed()
			}
			_ => SplitEventResult::ignored(),
		}
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

	fn is_selected(&self, row: u16, col: u16) -> bool {
		let Some(sel) = self.selection else {
			return false;
		};
		if !sel.contains(row, col) {
			return false;
		}
		let Some(screen) = self.screen() else {
			return true;
		};
		let (_, cols) = screen.size();
		for c in (col..cols).rev() {
			if let Some(cell) = screen.cell(row, c)
				&& cell.has_contents()
			{
				return c >= col;
			}
		}
		false
	}

	fn for_each_cell(&self, f: &mut dyn FnMut(u16, u16, &SplitCell)) {
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
