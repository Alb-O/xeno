use xeno_primitives::{Key, MouseEvent};

use super::dock::DockSlot;
use super::keymap::KeybindingRegistry;
use crate::impls::Editor;

/// Events that can be delivered to panels.
#[derive(Debug, Clone)]
pub enum UiEvent {
	/// Periodic tick for animations and async updates.
	Tick,
	/// Terminal window was resized.
	Resize,
	/// Keyboard input event.
	Key(Key),
	/// Mouse input event.
	Mouse(MouseEvent),
	/// Text pasted from clipboard.
	Paste(String),
}

/// Requests that panels can emit to the UI manager.
#[derive(Debug, Clone)]
pub enum UiRequest {
	/// Request a screen redraw.
	Redraw,
	/// Request focus change to a specific target.
	Focus(super::UiFocus),
	/// Request closing a panel by ID.
	ClosePanel(String),
	/// Request toggling a panel's open state by ID.
	TogglePanel(String),
}

/// Result returned from panel event handlers.
#[derive(Debug, Default)]
pub struct EventResult {
	/// Whether the event was consumed (stops further propagation).
	pub consumed: bool,
	/// UI requests to process after event handling.
	pub requests: Vec<UiRequest>,
}

impl EventResult {
	/// Creates a result indicating the event was consumed.
	pub fn consumed() -> Self {
		Self {
			consumed: true,
			requests: Vec::new(),
		}
	}

	/// Creates a result indicating the event was not consumed.
	pub fn not_consumed() -> Self {
		Self {
			consumed: false,
			requests: Vec::new(),
		}
	}

	/// Builder: adds a UI request to the result.
	pub fn with_request(mut self, req: UiRequest) -> Self {
		self.requests.push(req);
		self
	}
}

/// Context provided to panels during initialization.
pub struct PanelInitContext<'a> {
	/// Keybinding registry for registering panel-specific bindings.
	pub keybindings: &'a mut KeybindingRegistry,
}

/// Trait for UI panels that can be displayed in dock slots.
pub trait Panel {
	/// Returns the unique identifier for this panel.
	fn id(&self) -> &str;
	/// Returns the default dock slot for this panel.
	fn default_slot(&self) -> DockSlot;

	/// Called when the panel is registered with the UI manager.
	fn on_register(&mut self, _ctx: PanelInitContext<'_>) {}
	/// Called once during editor startup.
	fn on_startup(&mut self) {}

	/// Called when the panel's open state changes.
	fn on_open_changed(&mut self, _open: bool) {}
	/// Called when the panel gains or loses focus.
	fn on_focus_changed(&mut self, _focused: bool) {}

	/// Returns the cursor style to use when this panel is focused.
	fn cursor_style_when_focused(&self) -> Option<crate::runtime::CursorStyle> {
		None
	}

	/// Handles a UI event, returning whether it was consumed.
	fn handle_event(&mut self, event: UiEvent, editor: &mut Editor, focused: bool) -> EventResult;
}
