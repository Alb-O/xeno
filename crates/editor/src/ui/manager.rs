//! Central manager for the editor UI, coordinating panels, focus, and docking.

use std::collections::HashMap;

use xeno_primitives::{Key, MouseEvent};
use xeno_tui::layout::Rect;

use super::dock::{DockLayout, DockManager, DockSlot, SizeSpec};
use super::focus::{FocusManager, UiFocus};
use super::ids::UTILITY_PANEL_ID;
use super::keymap::{BindingScope, KeybindingRegistry};
use super::panel::{Panel, PanelInitContext, UiEvent, UiRequest};

/// Central coordinator for the editor UI subsystem.
///
/// Manages panel registration, dock layout, focus tracking, and event routing.
pub struct UiManager {
	/// Manages panel positions and layout constraints.
	pub dock: DockManager,
	/// Tracks which element (editor or panel) currently has focus.
	pub focus: FocusManager,
	/// Registry of keybindings for UI elements.
	pub keymap: KeybindingRegistry,
	/// Map of panel IDs to their implementations.
	panels: HashMap<String, Box<dyn Panel>>,
	/// Flag indicating the UI needs to be redrawn.
	wants_redraw: bool,
	/// True when the utility panel was auto-opened for which-key.
	utility_opened_for_whichkey: bool,
	/// True when utility height is currently auto-sized for which-key.
	utility_sized_for_whichkey: bool,
	/// True when the utility panel was auto-opened for a modal overlay.
	utility_opened_for_overlay: bool,
	/// True when utility height is currently forced for modal overlay.
	utility_sized_for_overlay: bool,
	/// Baseline utility panel size when not auto-sized.
	utility_default_size: SizeSpec,
}

impl Default for UiManager {
	fn default() -> Self {
		Self::new()
	}
}

impl UiManager {
	/// Creates a new UI manager with default dock configuration.
	pub fn new() -> Self {
		let utility_default_size = SizeSpec::Lines(10);
		let mut ui = Self {
			dock: DockManager::new(),
			focus: FocusManager::new(),
			keymap: KeybindingRegistry::new(),
			panels: HashMap::new(),
			wants_redraw: false,
			utility_opened_for_whichkey: false,
			utility_sized_for_whichkey: false,
			utility_opened_for_overlay: false,
			utility_sized_for_overlay: false,
			utility_default_size,
		};
		let _ = ui.dock.set_slot_size(DockSlot::Bottom, utility_default_size);
		ui.register_panel(Box::<super::panels::utility::UtilityPanel>::default());
		ui
	}

	fn set_utility_size(&mut self, size: SizeSpec) {
		if self.dock.set_slot_size(DockSlot::Bottom, size) {
			self.wants_redraw = true;
		}
	}

	/// Synchronizes utility panel visibility and size for which-key mode without stealing focus.
	pub fn sync_utility_for_whichkey(&mut self, desired_height: Option<u16>) {
		if let Some(height) = desired_height {
			if !self.dock.is_open(UTILITY_PANEL_ID) {
				self.set_open(UTILITY_PANEL_ID, true);
				self.utility_opened_for_whichkey = true;
			}
			if !self.utility_sized_for_overlay {
				self.set_utility_size(SizeSpec::Lines(height.clamp(4, 10)));
				self.utility_sized_for_whichkey = true;
			}
			return;
		}

		if self.utility_sized_for_whichkey {
			if !self.utility_sized_for_overlay {
				self.set_utility_size(self.utility_default_size);
			}
			self.utility_sized_for_whichkey = false;
		}

		if self.utility_opened_for_whichkey {
			if self.dock.is_open(UTILITY_PANEL_ID) && !self.utility_opened_for_overlay {
				self.set_open(UTILITY_PANEL_ID, false);
			}
			self.utility_opened_for_whichkey = false;
		}
	}

	/// Synchronizes utility panel visibility for modal overlays without stealing focus.
	pub fn sync_utility_for_modal_overlay(&mut self, desired_height: Option<u16>) {
		if let Some(height) = desired_height {
			if !self.dock.is_open(UTILITY_PANEL_ID) {
				self.set_open(UTILITY_PANEL_ID, true);
				self.utility_opened_for_overlay = true;
			}
			self.set_utility_size(SizeSpec::Lines(height.clamp(1, 10)));
			self.utility_sized_for_overlay = true;
			return;
		}

		if self.utility_sized_for_overlay {
			if self.utility_sized_for_whichkey {
				// Keep which-key-driven height when it is active.
			} else {
				self.set_utility_size(self.utility_default_size);
			}
			self.utility_sized_for_overlay = false;
		}

		if self.utility_opened_for_overlay {
			if self.dock.is_open(UTILITY_PANEL_ID) && !self.utility_opened_for_whichkey {
				self.set_open(UTILITY_PANEL_ID, false);
			}
			self.utility_opened_for_overlay = false;
		}
	}

	/// Registers a panel with the UI manager, calling its `on_register` hook.
	pub fn register_panel(&mut self, mut panel: Box<dyn Panel>) {
		let id = panel.id().to_string();
		panel.on_register(PanelInitContext { keybindings: &mut self.keymap });
		self.panels.insert(id, panel);
	}

	/// Calls `on_startup` for all registered panels.
	pub fn startup(&mut self) {
		let ids: Vec<String> = self.panels.keys().cloned().collect();
		for id in ids {
			if let Some(panel) = self.panels.get_mut(&id) {
				panel.on_startup();
			}
		}
	}

	/// Returns whether any panel is currently open in any dock slot.
	pub fn any_panel_open(&self) -> bool {
		self.dock.any_open()
	}

	/// Returns whether a panel with the given ID is registered.
	pub fn has_panel(&self, id: &str) -> bool {
		self.panels.contains_key(id)
	}

	/// Invokes `f` with a mutable panel reference when the panel is registered.
	pub fn with_panel_mut<R, F>(&mut self, id: &str, f: F) -> Option<R>
	where
		F: FnOnce(&mut dyn Panel) -> R,
	{
		self.panels.get_mut(id).map(|panel| f(panel.as_mut()))
	}

	/// Returns and clears the redraw flag, indicating if a redraw was requested.
	pub fn take_wants_redraw(&mut self) -> bool {
		let v = self.wants_redraw;
		self.wants_redraw = false;
		v
	}

	/// Returns the ID of the currently focused panel, if any.
	pub fn focused_panel_id(&self) -> Option<&str> {
		self.focus.focused().panel_id()
	}

	/// Returns whether the panel with the given ID is currently focused.
	pub fn is_panel_focused(&self, id: &str) -> bool {
		self.focused_panel_id() == Some(id)
	}

	/// Computes the dock layout for the given main area.
	pub fn compute_layout(&self, main_area: Rect) -> DockLayout {
		self.dock.compute_layout(main_area)
	}

	/// Returns the cursor style for the currently focused panel, if any.
	pub fn cursor_style(&self) -> Option<crate::runtime::CursorStyle> {
		let panel_id = self.focused_panel_id()?;
		self.panels.get(panel_id).and_then(|p| p.cursor_style_when_focused())
	}

	/// Handles a key event at the global scope, returning true if consumed.
	pub fn handle_global_key(&mut self, key: &Key) -> bool {
		let scope = BindingScope::Global;
		let binding = self.keymap.match_key(&scope, key);
		if let Some(binding) = binding {
			self.apply_requests(binding.requests.clone());
			return true;
		}
		false
	}

	/// Routes a key event to the focused panel, returning true if consumed.
	pub fn handle_focused_key(&mut self, editor: &mut crate::impls::Editor, key: Key) -> bool {
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

	/// Routes a paste event to the focused panel, returning true if consumed.
	pub fn handle_paste(&mut self, editor: &mut crate::impls::Editor, content: String) -> bool {
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

	/// Routes a mouse event to the appropriate panel based on hit testing.
	pub fn handle_mouse(&mut self, editor: &mut crate::impls::Editor, mouse: MouseEvent, layout: &DockLayout) -> bool {
		let row = mouse.row();
		let col = mouse.col();

		let mut hit_panel: Option<String> = None;
		for (id, area) in &layout.panel_areas {
			if row >= area.y && row < area.y + area.height && col >= area.x && col < area.x + area.width {
				hit_panel = Some(id.clone());
				break;
			}
		}

		if let Some(id) = hit_panel {
			// Focus follows mouse for panels.
			self.apply_requests(vec![UiRequest::Focus(UiFocus::panel(id.clone()))]);
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
			self.apply_requests(vec![UiRequest::Focus(UiFocus::editor())]);
		}
		false
	}

	/// Sends a tick event to all panels for periodic updates.
	pub fn tick(&mut self, editor: &mut crate::impls::Editor) {
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

	/// Notifies all panels of a terminal resize event.
	pub fn notify_resize(&mut self, editor: &mut crate::impls::Editor, _width: u16, _height: u16) {
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

	/// Sets whether a panel is open, updating dock state and focus as needed.
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
				self.focus.set_focused(UiFocus::editor());
			}
		}
		self.wants_redraw = true;
	}

	/// Toggles a panel's open state, focusing it when opened.
	pub fn toggle_panel(&mut self, id: &str) {
		let open = self.dock.is_open(id);
		self.set_open(id, !open);
		if !open {
			self.apply_requests(vec![UiRequest::Focus(UiFocus::panel(id.to_string()))]);
		}
	}

	/// Processes a batch of UI requests (focus changes, panel toggles, etc.).
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
}
