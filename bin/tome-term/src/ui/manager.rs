use std::collections::HashMap;

use ratatui::layout::Rect;
use termina::event::{KeyEvent, MouseEvent};

use super::dock::{DockLayout, DockManager};
use super::focus::{FocusManager, FocusTarget};
use super::keymap::{BindingScope, KeybindingRegistry};
use super::panel::{Panel, PanelInitContext, UiEvent, UiRequest};
use crate::theme::Theme;

#[derive(Default)]
pub struct UiManager {
	pub dock: DockManager,
	pub focus: FocusManager,
	pub keymap: KeybindingRegistry,
	panels: HashMap<String, Box<dyn Panel>>,
	wants_redraw: bool,
}

impl UiManager {
	pub fn new() -> Self {
		Self {
			dock: DockManager::new(),
			focus: FocusManager::new(),
			keymap: KeybindingRegistry::new(),
			panels: HashMap::new(),
			wants_redraw: false,
		}
	}

	pub fn register_panel(&mut self, mut panel: Box<dyn Panel>) {
		let id = panel.id().to_string();
		panel.on_register(PanelInitContext {
			keybindings: &mut self.keymap,
		});
		self.panels.insert(id, panel);
	}

	pub fn startup(&mut self) {
		let ids: Vec<String> = self.panels.keys().cloned().collect();
		for id in ids {
			if let Some(panel) = self.panels.get_mut(&id) {
				panel.on_startup();
			}
		}
	}

	pub fn any_panel_open(&self) -> bool {
		self.dock.any_open()
	}

	pub fn take_wants_redraw(&mut self) -> bool {
		let v = self.wants_redraw;
		self.wants_redraw = false;
		v
	}

	pub fn focused_panel_id(&self) -> Option<&str> {
		self.focus.focused().panel_id()
	}

	pub fn is_panel_focused(&self, id: &str) -> bool {
		self.focused_panel_id() == Some(id)
	}

	pub fn compute_layout(&self, main_area: Rect) -> DockLayout {
		self.dock.compute_layout(main_area)
	}

	pub fn cursor_style(&self) -> Option<termina::style::CursorStyle> {
		let panel_id = self.focused_panel_id()?;
		self.panels
			.get(panel_id)
			.and_then(|p| p.cursor_style_when_focused())
	}

	pub fn handle_global_key(&mut self, key: &KeyEvent) -> bool {
		let scope = BindingScope::Global;
		let binding = self.keymap.match_key(&scope, key);
		if let Some(binding) = binding {
			self.apply_requests(binding.requests.clone());
			return true;
		}
		false
	}

	pub fn handle_focused_key(
		&mut self,
		editor: &mut crate::editor::Editor,
		key: KeyEvent,
	) -> bool {
		let Some(panel_id) = self.focused_panel_id().map(|s| s.to_string()) else {
			return false;
		};

		let Some(panel) = self.panels.get_mut(&panel_id) else {
			return false;
		};

		let res = panel.handle_event(UiEvent::Key(key), editor, true);
		self.apply_requests(res.requests);
		res.consumed
	}

	pub fn handle_paste(&mut self, editor: &mut crate::editor::Editor, content: String) -> bool {
		let Some(panel_id) = self.focused_panel_id().map(|s| s.to_string()) else {
			return false;
		};
		let Some(panel) = self.panels.get_mut(&panel_id) else {
			return false;
		};
		let res = panel.handle_event(UiEvent::Paste(content), editor, true);
		self.apply_requests(res.requests);
		res.consumed
	}

	pub fn handle_mouse(
		&mut self,
		editor: &mut crate::editor::Editor,
		mouse: MouseEvent,
		layout: &DockLayout,
	) -> bool {
		let row = mouse.row;
		let col = mouse.column;

		let mut hit_panel: Option<String> = None;
		for (id, area) in &layout.panel_areas {
			if row >= area.y
				&& row < area.y + area.height
				&& col >= area.x
				&& col < area.x + area.width
			{
				hit_panel = Some(id.clone());
				break;
			}
		}

		if let Some(id) = hit_panel {
			// Focus follows mouse for panels.
			self.apply_requests(vec![UiRequest::Focus(FocusTarget::panel(id.clone()))]);
			let focused = self.is_panel_focused(&id);
			if let Some(panel) = self.panels.get_mut(&id) {
				let res = panel.handle_event(UiEvent::Mouse(mouse), editor, focused);
				self.apply_requests(res.requests);
				return res.consumed;
			}
			return true;
		}

		// Click outside any panel: if we were focused on a panel, return focus to editor.
		if self.focused_panel_id().is_some() {
			self.apply_requests(vec![UiRequest::Focus(FocusTarget::editor())]);
		}
		false
	}

	pub fn tick(&mut self, editor: &mut crate::editor::Editor) {
		let ids: Vec<String> = self.panels.keys().cloned().collect();
		let mut requests = Vec::new();
		for id in ids {
			let focused = self.is_panel_focused(&id);
			if let Some(panel) = self.panels.get_mut(&id) {
				let res = panel.handle_event(UiEvent::Tick, editor, focused);
				requests.extend(res.requests);
			}
		}
		self.apply_requests(requests);
	}

	pub fn notify_resize(&mut self, editor: &mut crate::editor::Editor, _width: u16, _height: u16) {
		let ids: Vec<String> = self.panels.keys().cloned().collect();
		let mut requests = Vec::new();
		for id in ids {
			let focused = self.is_panel_focused(&id);
			if let Some(panel) = self.panels.get_mut(&id) {
				let res = panel.handle_event(UiEvent::Resize, editor, focused);
				requests.extend(res.requests);
			}
		}
		self.apply_requests(requests);
	}

	pub fn set_open(&mut self, id: &str, open: bool) {
		let Some(panel) = self.panels.get_mut(id) else {
			return;
		};
		if open {
			self.dock.open_panel(panel.default_slot(), id.to_string());
			panel.on_open_changed(true);
		} else {
			self.dock.close_panel(id);
			panel.on_open_changed(false);
			if self.is_panel_focused(id) {
				self.focus.set_focused(FocusTarget::editor());
			}
		}
		self.wants_redraw = true;
	}

	pub fn toggle_panel(&mut self, id: &str) {
		let open = self.dock.is_open(id);
		self.set_open(id, !open);
		if !open {
			self.apply_requests(vec![UiRequest::Focus(FocusTarget::panel(id.to_string()))]);
		}
	}

	pub fn apply_requests(&mut self, requests: Vec<UiRequest>) {
		for req in requests {
			match req {
				UiRequest::Redraw => {
					self.wants_redraw = true;
				}
				UiRequest::Focus(target) => {
					let old = self.focus.focused().clone();
					if old != target {
						if let Some(old_id) = old.panel_id()
							&& let Some(panel) = self.panels.get_mut(old_id)
						{
							panel.on_focus_changed(false);
						}
						if let Some(new_id) = target.panel_id()
							&& let Some(panel) = self.panels.get_mut(new_id)
						{
							panel.on_focus_changed(true);
						}
						self.focus.set_focused(target);
						self.wants_redraw = true;
					}
				}
				UiRequest::ClosePanel(id) => self.set_open(&id, false),
				UiRequest::TogglePanel(id) => self.toggle_panel(&id),
			}
		}
	}

	pub fn render_panels(
		&mut self,
		editor: &mut crate::editor::Editor,
		frame: &mut ratatui::Frame,
		layout: &DockLayout,
		theme: &Theme,
	) -> Option<ratatui::layout::Position> {
		let mut cursor: Option<ratatui::layout::Position> = None;
		for (id, area) in &layout.panel_areas {
			let focused = self.is_panel_focused(id);
			if let Some(panel) = self.panels.get_mut(id) {
				let cursor_req = panel.render(frame, *area, editor, focused, theme);
				if focused && let Some(req) = cursor_req {
					cursor = Some(req.pos);
				}
			}
		}
		cursor
	}
}
