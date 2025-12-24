use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use termina::event::{KeyCode as TmKeyCode, Modifiers as TmModifiers};

use crate::acp::ChatRole;
use crate::theme::Theme;
use crate::ui::FocusTarget;
use crate::ui::dock::DockSlot;
use crate::ui::panel::{CursorRequest, EventResult, Panel, UiEvent, UiRequest};

pub fn chat_panel_ui_id(panel_id: u64) -> String {
	format!("chat:{}", panel_id)
}

pub struct AcpChatPanel {
	panel_id: u64,
	ui_id: String,
}

impl AcpChatPanel {
	pub fn new(panel_id: u64, _title: String) -> Self {
		Self {
			panel_id,
			ui_id: chat_panel_ui_id(panel_id),
		}
	}

	fn role_prefix(role: ChatRole) -> &'static str {
		match role {
			ChatRole::User => "You",
			ChatRole::Assistant => "Assistant",
			ChatRole::System => "System",
			ChatRole::Thought => "Thought",
		}
	}
}

impl Panel for AcpChatPanel {
	fn id(&self) -> &str {
		&self.ui_id
	}

	fn default_slot(&self) -> DockSlot {
		DockSlot::Bottom
	}

	fn cursor_style_when_focused(&self) -> Option<termina::style::CursorStyle> {
		Some(termina::style::CursorStyle::Default)
	}

	fn handle_event(
		&mut self,
		event: UiEvent,
		editor: &mut crate::editor::Editor,
		focused: bool,
	) -> EventResult {
		match event {
			UiEvent::Key(key) if focused => {
				let raw_ctrl_enter = matches!(
					key.code,
					TmKeyCode::Enter | TmKeyCode::Char('\n') | TmKeyCode::Char('j')
				) && key.modifiers.contains(TmModifiers::CONTROL);

				if raw_ctrl_enter {
					editor.submit_acp_panel();
					return EventResult::consumed().with_request(UiRequest::Redraw);
				}

				if key.code == TmKeyCode::Escape {
					return EventResult::consumed()
						.with_request(UiRequest::Focus(FocusTarget::editor()));
				}

				let mut panels = editor.acp.state.panels.lock();
				let Some(panel) = panels.get_mut(&self.panel_id) else {
					return EventResult::consumed();
				};

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
					TmKeyCode::Left => {
						panel.input_cursor = panel.input_cursor.saturating_sub(1);
					}
					TmKeyCode::Right => {
						panel.input_cursor = (panel.input_cursor + 1).min(panel.input.len_chars());
					}
					TmKeyCode::Enter => {
						panel.input.insert(panel.input_cursor, "\n");
						panel.input_cursor += 1;
					}
					TmKeyCode::Tab => {
						panel.input.insert(panel.input_cursor, "\t");
						panel.input_cursor += 1;
					}
					_ => {}
				}

				EventResult::consumed().with_request(UiRequest::Redraw)
			}
			UiEvent::Paste(text) if focused => {
				let mut panels = editor.acp.state.panels.lock();
				let Some(panel) = panels.get_mut(&self.panel_id) else {
					return EventResult::consumed();
				};
				panel.input.insert(panel.input_cursor, &text);
				panel.input_cursor += text.chars().count();
				EventResult::consumed().with_request(UiRequest::Redraw)
			}
			_ => EventResult::not_consumed(),
		}
	}

	fn render(
		&mut self,
		frame: &mut ratatui::Frame<'_>,
		area: Rect,
		editor: &mut crate::editor::Editor,
		focused: bool,
		theme: &Theme,
	) -> Option<CursorRequest> {
		let bg = Style::default()
			.bg(theme.colors.popup.bg)
			.fg(theme.colors.popup.fg);
		let user_style = Style::default()
			.fg(theme.colors.ui.fg)
			.add_modifier(Modifier::BOLD);
		let assistant_style = Style::default().fg(theme.colors.ui.fg);
		let input_style = Style::default().fg(theme.colors.ui.command_input_fg);

		frame.render_widget(Block::default().style(bg), area);

		let panels = editor.acp.state.panels.lock();
		let Some(state) = panels.get(&self.panel_id) else {
			let w = Paragraph::new("[missing panel state]").style(bg);
			frame.render_widget(w, area);
			return None;
		};

		let chunks = Layout::default()
			.direction(Direction::Vertical)
			.constraints([Constraint::Min(1), Constraint::Length(1)])
			.split(area);
		let transcript_area = chunks[0];
		let input_area = chunks[1];

		let mut transcript_lines: Vec<Line> = Vec::new();
		for item in &state.transcript {
			let role = Self::role_prefix(item.role);
			let style = match item.role {
				ChatRole::User => user_style,
				_ => assistant_style,
			};

			for (i, line) in item.text.lines().enumerate() {
				if i == 0 {
					transcript_lines.push(Line::from(vec![
						Span::styled(format!("{}: ", role), style),
						Span::styled(line.to_string(), bg),
					]));
				} else {
					transcript_lines.push(Line::from(vec![Span::styled(line.to_string(), bg)]));
				}
			}
		}

		// Keep only the bottom-most lines that fit.
		if transcript_lines.len() > transcript_area.height as usize {
			let start = transcript_lines.len() - transcript_area.height as usize;
			transcript_lines = transcript_lines.split_off(start);
		}

		frame.render_widget(Paragraph::new(transcript_lines).style(bg), transcript_area);

		let input_text = state.input.to_string();
		let prompt = "> ";
		let last_line = input_text.lines().last().unwrap_or("");
		let input_line = Line::from(vec![
			Span::styled(prompt, input_style),
			Span::styled(last_line.to_string(), input_style),
		]);
		frame.render_widget(Paragraph::new(vec![input_line]).style(bg), input_area);

		if focused {
			// Approximate cursor position: only supports showing cursor on last line.
			let last_nl_char_idx = input_text
				.rfind('\n')
				.map(|byte_idx| input_text[..=byte_idx].chars().count())
				.unwrap_or(0);
			let cursor_col = state.input_cursor.saturating_sub(last_nl_char_idx);
			let x = input_area
				.x
				.saturating_add(prompt.chars().count() as u16)
				.saturating_add(cursor_col as u16)
				.min(input_area.x + input_area.width.saturating_sub(1));
			return Some(CursorRequest {
				pos: Position { x, y: input_area.y },
				style: Some(termina::style::CursorStyle::Default),
			});
		}

		None
	}
}
