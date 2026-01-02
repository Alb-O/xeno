//! Focus management for tracking which element has keyboard focus.

/// The type of element that can receive focus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FocusTargetKind {
	/// The main text editor.
	Editor,
	/// A panel identified by its ID.
	Panel(String),
}

/// Identifies which element currently has keyboard focus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusTarget(pub FocusTargetKind);

impl FocusTarget {
	/// Creates a focus target for the main editor.
	pub fn editor() -> Self {
		Self(FocusTargetKind::Editor)
	}

	/// Creates a focus target for a panel with the given ID.
	pub fn panel(id: impl Into<String>) -> Self {
		Self(FocusTargetKind::Panel(id.into()))
	}

	/// Returns whether this target is the main editor.
	pub fn is_editor(&self) -> bool {
		matches!(self.0, FocusTargetKind::Editor)
	}

	/// Returns the panel ID if this is a panel target.
	pub fn panel_id(&self) -> Option<&str> {
		match &self.0 {
			FocusTargetKind::Panel(id) => Some(id.as_str()),
			_ => None,
		}
	}
}

/// Tracks which element in the UI currently has keyboard focus.
#[derive(Debug)]
pub struct FocusManager {
	/// The element that currently has focus.
	focused: FocusTarget,
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
			focused: FocusTarget::editor(),
		}
	}

	/// Returns the currently focused element.
	pub fn focused(&self) -> &FocusTarget {
		&self.focused
	}

	/// Sets the focused element.
	pub fn set_focused(&mut self, target: FocusTarget) {
		self.focused = target;
	}
}
