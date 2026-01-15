//! Focus management for tracking which UI element has keyboard focus.
//!
//! This tracks high-level focus zones (editor area vs UI panels), as opposed to
//! `editor::FocusTarget` which tracks specific buffers within the editor.

/// The kind of UI element that can receive focus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UiFocusKind {
	/// The main text editor area.
	Editor,
	/// A panel identified by its ID.
	Panel(String),
}

/// Identifies which UI zone currently has keyboard focus.
///
/// This is a coarse-grained focus tracker for distinguishing between the editor
/// and UI panels. For tracking specific buffers within the editor, see
/// `editor::FocusTarget`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiFocus(pub UiFocusKind);

impl UiFocus {
	/// Creates a focus target for the main editor.
	pub fn editor() -> Self {
		Self(UiFocusKind::Editor)
	}

	/// Creates a focus target for a panel with the given ID.
	pub fn panel(id: impl Into<String>) -> Self {
		Self(UiFocusKind::Panel(id.into()))
	}

	/// Returns whether this target is the main editor.
	pub fn is_editor(&self) -> bool {
		matches!(self.0, UiFocusKind::Editor)
	}

	/// Returns the panel ID if this is a panel target.
	pub fn panel_id(&self) -> Option<&str> {
		match &self.0 {
			UiFocusKind::Panel(id) => Some(id.as_str()),
			_ => None,
		}
	}
}

/// Tracks which UI zone currently has keyboard focus.
#[derive(Debug)]
pub struct FocusManager {
	/// The element that currently has focus.
	focused: UiFocus,
}

impl Default for FocusManager {
	fn default() -> Self {
		Self::new()
	}
}

impl FocusManager {
	/// Creates a new focus manager with focus on the editor.
	pub fn new() -> Self {
		Self {
			focused: UiFocus::editor(),
		}
	}

	/// Returns the currently focused element.
	pub fn focused(&self) -> &UiFocus {
		&self.focused
	}

	/// Sets the focused element.
	pub fn set_focused(&mut self, target: UiFocus) {
		self.focused = target;
	}
}
