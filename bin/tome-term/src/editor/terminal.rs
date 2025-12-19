use std::sync::mpsc::TryRecvError;

use crate::editor::Editor;
use crate::terminal_panel::TerminalState;

impl Editor {
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
		if self.terminal_open {
			if self.terminal_focused {
				self.terminal_focused = false;
				self.terminal_focus_pending = false;
			} else {
				self.start_terminal_prewarm();
				self.terminal_focus_pending = true;
			}
			return;
		}

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

	pub(crate) fn handle_terminal_key(&mut self, key: &termina::event::KeyEvent) -> bool {
		use termina::event::{KeyCode as TmKeyCode, Modifiers as TmModifiers};

		if self.terminal_open && self.terminal_focused {
			// Esc to exit terminal focus (but keep open)
			if matches!(key.code, TmKeyCode::Escape) {
				self.terminal_focused = false;
				self.terminal_focus_pending = false;
				self.terminal_input_buffer.clear();
				return true;
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

			return true;
		}
		false
	}

	pub(crate) fn handle_terminal_mouse(&mut self, mouse: &termina::event::MouseEvent) -> bool {
		let height = self.window_height.unwrap_or(24);

		if self.terminal_open {
			// Terminal takes bottom 30% of main area, leaving 2 rows for status and message
			let main_area_height = height.saturating_sub(2);
			let doc_height = (main_area_height * 70) / 100;
			let term_start = doc_height;
			let term_end = main_area_height;

			if mouse.row >= term_start && mouse.row < term_end {
				// Click is in terminal area - focus it and swallow the event
				if !self.terminal_focused {
					self.terminal_focused = true;
				}
				// Terminal doesn't process mouse events yet, just swallow them
				return true;
			} else if self.terminal_focused {
				// Click outside terminal while focused - unfocus it
				self.terminal_focused = false;
				// Fall through to process click in main editor
			}
		}
		false
	}
}
