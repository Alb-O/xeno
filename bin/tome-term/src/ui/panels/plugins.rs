use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use termina::event::{KeyCode as TmKeyCode, KeyEvent};

use crate::theme::Theme;
use crate::ui::FocusTarget;
use crate::ui::dock::DockSlot;
use crate::ui::panel::{CursorRequest, EventResult, Panel, UiEvent, UiRequest};

pub const PLUGINS_PANEL_ID: &str = "plugins";

pub struct PluginsPanel {
	selected_idx: usize,
}

impl PluginsPanel {
	pub fn new() -> Self {
		Self { selected_idx: 0 }
	}

	fn sorted_plugin_ids(editor: &crate::editor::Editor) -> Vec<String> {
		let mut ids: Vec<String> = editor.plugins.entries.keys().cloned().collect();
		ids.sort();
		ids
	}
}

impl Panel for PluginsPanel {
	fn id(&self) -> &str {
		PLUGINS_PANEL_ID
	}

	fn default_slot(&self) -> DockSlot {
		DockSlot::Left
	}

	fn cursor_style_when_focused(&self) -> Option<termina::style::CursorStyle> {
		// Panels use the terminal cursor; keep user's default.
		Some(termina::style::CursorStyle::Default)
	}

	fn handle_event(
		&mut self,
		event: UiEvent,
		editor: &mut crate::editor::Editor,
		focused: bool,
	) -> EventResult {
		match event {
			UiEvent::Key(KeyEvent { code, .. }) if focused => {
				let ids = Self::sorted_plugin_ids(editor);
				if ids.is_empty() {
					match code {
						TmKeyCode::Escape | TmKeyCode::Char('q') => {
							return EventResult::consumed()
								.with_request(UiRequest::ClosePanel(PLUGINS_PANEL_ID.to_string()))
								.with_request(UiRequest::Focus(FocusTarget::editor()));
						}
						_ => return EventResult::consumed(),
					}
				}

				self.selected_idx = self.selected_idx.min(ids.len().saturating_sub(1));

				match code {
					TmKeyCode::Char('j') | TmKeyCode::Down => {
						self.selected_idx = (self.selected_idx + 1) % ids.len();
						EventResult::consumed().with_request(UiRequest::Redraw)
					}
					TmKeyCode::Char('k') | TmKeyCode::Up => {
						self.selected_idx = self
							.selected_idx
							.checked_sub(1)
							.unwrap_or(ids.len().saturating_sub(1));
						EventResult::consumed().with_request(UiRequest::Redraw)
					}
					TmKeyCode::Enter | TmKeyCode::Char(' ') => {
						let id = &ids[self.selected_idx];
						let enabled = editor.plugins.config.plugins.enabled.contains(id);
						if enabled {
							let _ = editor.plugin_command(&["disable", id]);
						} else {
							let _ = editor.plugin_command(&["enable", id]);
						}
						EventResult::consumed().with_request(UiRequest::Redraw)
					}
					TmKeyCode::Char('r') => {
						let id = &ids[self.selected_idx];
						let _ = editor.plugin_command(&["reload", id]);
						EventResult::consumed().with_request(UiRequest::Redraw)
					}
					TmKeyCode::Escape | TmKeyCode::Char('q') => EventResult::consumed()
						.with_request(UiRequest::ClosePanel(PLUGINS_PANEL_ID.to_string()))
						.with_request(UiRequest::Focus(FocusTarget::editor())),
					_ => EventResult::consumed(),
				}
			}
			_ => EventResult::not_consumed(),
		}
	}

	fn render(
		&mut self,
		frame: &mut ratatui::Frame<'_>,
		area: Rect,
		editor: &mut crate::editor::Editor,
		_focused: bool,
		theme: &Theme,
	) -> Option<CursorRequest> {
		let bg = Style::default()
			.bg(theme.colors.popup.bg)
			.fg(theme.colors.popup.fg);
		let selected_style = Style::default()
			.bg(theme.colors.ui.selection_bg)
			.fg(theme.colors.ui.selection_fg)
			.add_modifier(Modifier::BOLD);

		let ids = Self::sorted_plugin_ids(editor);
		self.selected_idx = self.selected_idx.min(ids.len().saturating_sub(1));

		let mut lines: Vec<Line> = Vec::new();
		if ids.is_empty() {
			lines.push(Line::from(vec![Span::styled("No plugins found", bg)]));
		} else {
			for (i, id) in ids.iter().enumerate() {
				let entry = editor.plugins.entries.get(id);
				let enabled = editor.plugins.config.plugins.enabled.contains(id);
				let loaded = editor.plugins.plugins.contains_key(id);

				let status = if enabled {
					if loaded { "Enabled" } else { "Enabled*" }
				} else {
					"Disabled"
				};
				let name = entry
					.map(|e| e.manifest.name.as_str())
					.unwrap_or("<unknown>");
				let version = entry.map(|e| e.manifest.version.as_str()).unwrap_or("?");

				let prefix = if i == self.selected_idx { ">" } else { " " };
				let text = format!("{} {:<8} {:<12} {} ({})", prefix, status, id, name, version);
				let style = if i == self.selected_idx {
					selected_style
				} else {
					bg
				};
				lines.push(Line::from(vec![Span::styled(text, style)]));
			}
		}

		let block = Block::default().style(bg);
		frame.render_widget(block, area);
		frame.render_widget(Paragraph::new(lines).style(bg), area);
		None
	}
}
