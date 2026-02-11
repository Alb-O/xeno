//! Dock layout system for managing panel positions around the editor.
//!
//! The dock system organizes panels into slots (left, right, top, bottom, overlay)
//! and computes their layout constraints relative to the main document area.

use std::collections::HashMap;

use xeno_tui::layout::{Constraint, Direction, Layout, Rect};

/// Position where a panel can be docked in the editor layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockSlot {
	/// Left side of the editor, typically for file trees or sidebars.
	Left,
	/// Right side of the editor, typically for outlines or auxiliary panels.
	Right,
	/// Bottom of the editor, typically for terminals or output panels.
	Bottom,
	/// Top of the editor, typically for toolbars or status displays.
	Top,
	/// Floating overlay that covers the main content area.
	Overlay,
}

/// Specification for the size of a docked panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeSpec {
	/// Size as a percentage of the available space (0-100).
	Percent(u16),
	/// Size as a fixed number of terminal lines/columns.
	Lines(u16),
}

impl SizeSpec {
	/// Converts this size specification to a layout constraint.
	fn to_constraint(self) -> Constraint {
		match self {
			SizeSpec::Percent(p) => Constraint::Percentage(p),
			SizeSpec::Lines(lines) => Constraint::Length(lines),
		}
	}
}

/// State of a single dock slot, tracking open panels and the active one.
#[derive(Debug, Clone)]
pub struct DockSlotState {
	/// Size specification for this slot when panels are open.
	pub size: SizeSpec,
	/// List of panel IDs currently open in this slot.
	pub open: Vec<String>,
	/// ID of the currently active (visible) panel in this slot.
	pub active: Option<String>,
}

impl DockSlotState {
	/// Creates a new dock slot state with the given size and no open panels.
	pub fn new(size: SizeSpec) -> Self {
		Self {
			size,
			open: Vec::new(),
			active: None,
		}
	}
}

/// Manages the dock layout system, tracking which panels are open in each slot.
#[derive(Debug, Default)]
pub struct DockManager {
	/// Map from dock positions to their current state.
	pub slots: HashMap<DockSlot, DockSlotState>,
}

/// Computed layout result from the dock manager.
#[derive(Debug, Default)]
pub struct DockLayout {
	/// The remaining area for the main document after panels are laid out.
	pub doc_area: Rect,
	/// Map from panel IDs to their computed screen rectangles.
	pub panel_areas: HashMap<String, Rect>,
}

impl DockManager {
	/// Creates a new dock manager with default slot sizes.
	pub fn new() -> Self {
		let mut slots = HashMap::new();
		slots.insert(DockSlot::Bottom, DockSlotState::new(SizeSpec::Lines(10)));
		slots.insert(DockSlot::Top, DockSlotState::new(SizeSpec::Percent(25)));
		slots.insert(DockSlot::Left, DockSlotState::new(SizeSpec::Percent(25)));
		slots.insert(DockSlot::Right, DockSlotState::new(SizeSpec::Percent(25)));
		slots.insert(DockSlot::Overlay, DockSlotState::new(SizeSpec::Percent(100)));
		Self { slots }
	}

	/// Opens a panel in the specified dock slot, making it active.
	pub fn open_panel(&mut self, slot: DockSlot, id: String) {
		let state = self.slots.entry(slot).or_insert_with(|| DockSlotState::new(SizeSpec::Percent(30)));
		if !state.open.iter().any(|p| p == &id) {
			state.open.push(id.clone());
		}
		state.active = Some(id);
	}

	/// Closes a panel by ID, removing it from whichever slot contains it.
	pub fn close_panel(&mut self, id: &str) {
		for (_slot, state) in self.slots.iter_mut() {
			if let Some(pos) = state.open.iter().position(|p| p == id) {
				state.open.remove(pos);
				if state.active.as_deref() == Some(id) {
					state.active = state.open.last().cloned();
				}
				break;
			}
		}
	}

	/// Returns whether a panel with the given ID is currently open in any slot.
	pub fn is_open(&self, id: &str) -> bool {
		self.slots.values().any(|s| s.open.iter().any(|p| p == id))
	}

	/// Returns whether any panel is currently open in any slot.
	pub fn any_open(&self) -> bool {
		self.slots.values().any(|s| !s.open.is_empty())
	}

	/// Returns the size spec currently configured for a dock slot.
	pub fn slot_size(&self, slot: DockSlot) -> Option<SizeSpec> {
		self.slots.get(&slot).map(|state| state.size)
	}

	/// Sets the size spec for a dock slot, returning true when it changes.
	pub fn set_slot_size(&mut self, slot: DockSlot, size: SizeSpec) -> bool {
		let state = self.slots.entry(slot).or_insert_with(|| DockSlotState::new(SizeSpec::Percent(30)));
		if state.size == size {
			return false;
		}
		state.size = size;
		true
	}

	/// Returns the ID of the active panel in the given slot, if any.
	pub fn active_in_slot(&self, slot: DockSlot) -> Option<&str> {
		self.slots.get(&slot).and_then(|s| s.active.as_deref())
	}

	/// Computes the layout for all open panels within the given area.
	///
	/// Returns a `DockLayout` containing the remaining document area and
	/// the computed rectangles for each active panel.
	pub fn compute_layout(&self, area: Rect) -> DockLayout {
		let mut layout = DockLayout {
			doc_area: area,
			..Default::default()
		};

		let has_top = self.slots.get(&DockSlot::Top).map(|s| !s.open.is_empty()).unwrap_or(false);
		let has_bottom = self.slots.get(&DockSlot::Bottom).map(|s| !s.open.is_empty()).unwrap_or(false);

		let mut vertical_parts = vec![area];
		let mut top_area = None;
		let mut bottom_area = None;

		if has_top || has_bottom {
			let top_c = if has_top {
				self.slots
					.get(&DockSlot::Top)
					.map(|s| s.size.to_constraint())
					.unwrap_or(Constraint::Percentage(25))
			} else {
				Constraint::Length(0)
			};
			let bottom_c = if has_bottom {
				self.slots
					.get(&DockSlot::Bottom)
					.map(|s| s.size.to_constraint())
					.unwrap_or(Constraint::Percentage(30))
			} else {
				Constraint::Length(0)
			};

			vertical_parts = Layout::default()
				.direction(Direction::Vertical)
				.constraints([top_c, Constraint::Min(1), bottom_c])
				.split(area)
				.to_vec();

			if has_top {
				top_area = Some(vertical_parts[0]);
			}
			if has_bottom {
				bottom_area = Some(vertical_parts[2]);
			}
			layout.doc_area = vertical_parts[1];
		}

		let has_left = self.slots.get(&DockSlot::Left).map(|s| !s.open.is_empty()).unwrap_or(false);
		let has_right = self.slots.get(&DockSlot::Right).map(|s| !s.open.is_empty()).unwrap_or(false);

		if has_left || has_right {
			let left_c = if has_left {
				self.slots
					.get(&DockSlot::Left)
					.map(|s| s.size.to_constraint())
					.unwrap_or(Constraint::Percentage(25))
			} else {
				Constraint::Length(0)
			};
			let right_c = if has_right {
				self.slots
					.get(&DockSlot::Right)
					.map(|s| s.size.to_constraint())
					.unwrap_or(Constraint::Percentage(25))
			} else {
				Constraint::Length(0)
			};

			let parts = Layout::default()
				.direction(Direction::Horizontal)
				.constraints([left_c, Constraint::Min(1), right_c])
				.split(layout.doc_area);
			if has_left && let Some(id) = self.active_in_slot(DockSlot::Left) {
				layout.panel_areas.insert(id.to_string(), parts[0]);
			}
			if has_right && let Some(id) = self.active_in_slot(DockSlot::Right) {
				layout.panel_areas.insert(id.to_string(), parts[2]);
			}
			layout.doc_area = parts[1];
		}

		if let Some(area) = top_area
			&& let Some(id) = self.active_in_slot(DockSlot::Top)
		{
			layout.panel_areas.insert(id.to_string(), area);
		}
		if let Some(area) = bottom_area
			&& let Some(id) = self.active_in_slot(DockSlot::Bottom)
		{
			layout.panel_areas.insert(id.to_string(), area);
		}

		layout
	}
}

#[cfg(test)]
mod tests {
	use xeno_tui::layout::Rect;

	use super::{DockManager, DockSlot, SizeSpec};

	#[test]
	fn bottom_slot_defaults_to_fixed_lines() {
		let dock = DockManager::new();
		let bottom = dock.slots.get(&DockSlot::Bottom).expect("bottom slot should exist");
		assert_eq!(bottom.size, SizeSpec::Lines(10));
	}

	#[test]
	fn fixed_bottom_height_reduces_doc_area_deterministically() {
		let mut dock = DockManager::new();
		dock.open_panel(DockSlot::Bottom, "utility".to_string());

		let area = Rect::new(0, 0, 100, 40);
		let layout = dock.compute_layout(area);

		assert_eq!(layout.doc_area.height, 30);
		assert_eq!(layout.doc_area.y, 0);
		assert_eq!(layout.panel_areas.get("utility").map(|r| r.height), Some(10));
	}

	#[test]
	fn fixed_bottom_height_clamps_under_tiny_viewports() {
		let mut dock = DockManager::new();
		dock.open_panel(DockSlot::Bottom, "utility".to_string());

		let area = Rect::new(0, 0, 80, 8);
		let layout = dock.compute_layout(area);

		assert_eq!(layout.doc_area.height, 0);
		assert_eq!(layout.panel_areas.get("utility").map(|r| r.height), Some(8));
	}
}
