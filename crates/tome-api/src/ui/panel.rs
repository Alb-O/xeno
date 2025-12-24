use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use termina::event::{KeyEvent, MouseEvent};

use super::dock::DockSlot;
use super::keymap::KeybindingRegistry;
use crate::editor::Editor;
use crate::theme::Theme;

#[derive(Debug, Clone)]
pub enum UiEvent {
	Tick,
	Resize,
	Key(KeyEvent),
	Mouse(MouseEvent),
	Paste(String),
}

#[derive(Debug, Clone)]
pub enum UiRequest {
	Redraw,
	Focus(super::FocusTarget),
	ClosePanel(String),
	TogglePanel(String),
}

#[derive(Debug, Default)]
pub struct EventResult {
	pub consumed: bool,
	pub requests: Vec<UiRequest>,
}

impl EventResult {
	pub fn consumed() -> Self {
		Self {
			consumed: true,
			requests: Vec::new(),
		}
	}

	pub fn not_consumed() -> Self {
		Self {
			consumed: false,
			requests: Vec::new(),
		}
	}

	pub fn with_request(mut self, req: UiRequest) -> Self {
		self.requests.push(req);
		self
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorRequest {
	pub pos: Position,
	pub style: Option<termina::style::CursorStyle>,
}

pub struct PanelInitContext<'a> {
	pub keybindings: &'a mut KeybindingRegistry,
}

pub trait Panel {
	fn id(&self) -> &str;
	fn default_slot(&self) -> DockSlot;

	fn on_register(&mut self, _ctx: PanelInitContext<'_>) {}
	fn on_startup(&mut self) {}

	fn on_open_changed(&mut self, _open: bool) {}
	fn on_focus_changed(&mut self, _focused: bool) {}

	fn cursor_style_when_focused(&self) -> Option<termina::style::CursorStyle> {
		None
	}

	fn handle_event(&mut self, event: UiEvent, editor: &mut Editor, focused: bool) -> EventResult;

	fn render(
		&mut self,
		frame: &mut Frame<'_>,
		area: Rect,
		editor: &mut Editor,
		focused: bool,
		theme: &Theme,
	) -> Option<CursorRequest>;
}
