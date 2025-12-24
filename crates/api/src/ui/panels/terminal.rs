use std::sync::mpsc::{Receiver, TryRecvError};

use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear};
use termina::event::{KeyCode as TmKeyCode, KeyEvent, Modifiers as TmModifiers};

use crate::render::terminal::ThemedVt100Terminal;
use crate::terminal_panel::{TerminalError, TerminalState};
use crate::theme::Theme;
use crate::ui::FocusTarget;
use crate::ui::dock::DockSlot;
use crate::ui::keymap::UiKeyChord;
use crate::ui::panel::{CursorRequest, EventResult, Panel, PanelInitContext, UiEvent, UiRequest};

pub const TERMINAL_PANEL_ID: &str = "terminal";

pub struct TerminalPanel {
	id: String,
	terminal: Option<TerminalState>,
	prewarm: Option<Receiver<Result<TerminalState, TerminalError>>>,
	input_buffer: Vec<u8>,
}

impl TerminalPanel {
	pub fn new() -> Self {
		Self {
			id: TERMINAL_PANEL_ID.to_string(),
			terminal: None,
			prewarm: None,
			input_buffer: Vec::new(),
		}
	}

	fn start_prewarm(&mut self) {
		if self.terminal.is_some() || self.prewarm.is_some() {
			return;
		}
		let (tx, rx) = std::sync::mpsc::channel();
		self.prewarm = Some(rx);
		std::thread::spawn(move || {
			let _ = tx.send(TerminalState::new(80, 24));
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

	fn handle_key_focused(&mut self, key: &KeyEvent) {
		// Esc exits terminal focus (but keeps panel open).
		if matches!(key.code, TmKeyCode::Escape) {
			return;
		}

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

		if bytes.is_empty() {
			return;
		}

		if let Some(term) = &mut self.terminal {
			let _ = term.write_key(&bytes);
		} else {
			self.input_buffer.extend_from_slice(&bytes);
		}
	}
}

impl Panel for TerminalPanel {
	fn id(&self) -> &str {
		&self.id
	}

	fn default_slot(&self) -> DockSlot {
		DockSlot::Bottom
	}

	fn on_register(&mut self, ctx: PanelInitContext<'_>) {
		// Ctrl+t toggles the terminal panel.
		ctx.keybindings.register_global(
			UiKeyChord::ctrl_char('t'),
			100,
			vec![UiRequest::TogglePanel(self.id.clone())],
		);
	}

	fn on_startup(&mut self) {
		// Keep a shell warm so opening is instant.
		self.start_prewarm();
	}

	fn on_open_changed(&mut self, open: bool) {
		if open {
			self.start_prewarm();
		} else {
			self.input_buffer.clear();
		}
	}

	fn cursor_style_when_focused(&self) -> Option<termina::style::CursorStyle> {
		Some(termina::style::CursorStyle::Default)
	}

	fn handle_event(
		&mut self,
		event: UiEvent,
		_editor: &mut crate::editor::Editor,
		_focused: bool,
	) -> EventResult {
		match event {
			UiEvent::Tick => {
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
					return EventResult::consumed()
						.with_request(UiRequest::ClosePanel(self.id.clone()))
						.with_request(UiRequest::Redraw);
				}

				if changed {
					return EventResult::not_consumed().with_request(UiRequest::Redraw);
				}
				EventResult::not_consumed()
			}
			UiEvent::Key(key) => {
				if matches!(key.code, TmKeyCode::Escape) {
					return EventResult::consumed()
						.with_request(UiRequest::Focus(FocusTarget::editor()));
				}
				self.handle_key_focused(&key);
				EventResult::consumed().with_request(UiRequest::Redraw)
			}
			UiEvent::Paste(content) => {
				if let Some(term) = &mut self.terminal {
					let _ = term.write_key(content.as_bytes());
				} else {
					self.input_buffer.extend_from_slice(content.as_bytes());
				}
				EventResult::consumed().with_request(UiRequest::Redraw)
			}
			UiEvent::Mouse(_mouse) => {
				// Terminal mouse support is not implemented yet; swallow clicks.
				EventResult::consumed()
			}
			UiEvent::Resize => EventResult::not_consumed(),
		}
	}

	fn render(
		&mut self,
		frame: &mut ratatui::Frame<'_>,
		area: Rect,
		_editor: &mut crate::editor::Editor,
		focused: bool,
		theme: &Theme,
	) -> Option<CursorRequest> {
		if area.width == 0 || area.height == 0 {
			return None;
		}

		frame.render_widget(Clear, area);
		let base_style = Style::default()
			.bg(theme.colors.popup.bg)
			.fg(theme.colors.popup.fg);
		frame.render_widget(Block::default().style(base_style), area);

		if let Some(term) = &mut self.terminal {
			let (rows, cols) = term.screen().size();
			if rows != area.height || cols != area.width {
				let _ = term.resize(area.width, area.height);
			}

			let screen = term.screen();
			let term_widget = ThemedVt100Terminal::new(screen, base_style);
			frame.render_widget(term_widget, area);

			if focused && !screen.hide_cursor() {
				let (cur_row, cur_col) = screen.cursor_position();
				if cur_row < area.height && cur_col < area.width {
					return Some(CursorRequest {
						pos: Position {
							x: area.x + cur_col,
							y: area.y + cur_row,
						},
						style: Some(termina::style::CursorStyle::Default),
					});
				}
			}
		}

		None
	}
}
