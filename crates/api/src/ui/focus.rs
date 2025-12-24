#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FocusTargetKind {
	Editor,
	Panel(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusTarget(pub FocusTargetKind);

impl FocusTarget {
	pub fn editor() -> Self {
		Self(FocusTargetKind::Editor)
	}

	pub fn panel(id: impl Into<String>) -> Self {
		Self(FocusTargetKind::Panel(id.into()))
	}

	pub fn is_editor(&self) -> bool {
		matches!(self.0, FocusTargetKind::Editor)
	}

	pub fn panel_id(&self) -> Option<&str> {
		match &self.0 {
			FocusTargetKind::Panel(id) => Some(id.as_str()),
			_ => None,
		}
	}
}

#[derive(Debug)]
pub struct FocusManager {
	focused: FocusTarget,
}

impl Default for FocusManager {
	fn default() -> Self {
		Self::new()
	}
}

impl FocusManager {
	pub fn new() -> Self {
		Self {
			focused: FocusTarget::editor(),
		}
	}

	pub fn focused(&self) -> &FocusTarget {
		&self.focused
	}

	pub fn set_focused(&mut self, target: FocusTarget) {
		self.focused = target;
	}
}
