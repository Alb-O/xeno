use std::collections::HashMap;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockSlot {
	Left,
	Right,
	Bottom,
	Top,
	Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeSpec {
	Percent(u16),
}

impl SizeSpec {
	fn to_constraint(self) -> Constraint {
		match self {
			SizeSpec::Percent(p) => Constraint::Percentage(p),
		}
	}
}

#[derive(Debug, Clone)]
pub struct DockSlotState {
	pub size: SizeSpec,
	pub open: Vec<String>,
	pub active: Option<String>,
}

impl DockSlotState {
	pub fn new(size: SizeSpec) -> Self {
		Self {
			size,
			open: Vec::new(),
			active: None,
		}
	}
}

#[derive(Debug, Default)]
pub struct DockManager {
	pub slots: HashMap<DockSlot, DockSlotState>,
}

#[derive(Debug, Default)]
pub struct DockLayout {
	pub doc_area: Rect,
	pub panel_areas: HashMap<String, Rect>,
}

impl DockManager {
	pub fn new() -> Self {
		let mut slots = HashMap::new();
		slots.insert(DockSlot::Bottom, DockSlotState::new(SizeSpec::Percent(30)));
		slots.insert(DockSlot::Top, DockSlotState::new(SizeSpec::Percent(25)));
		slots.insert(DockSlot::Left, DockSlotState::new(SizeSpec::Percent(25)));
		slots.insert(DockSlot::Right, DockSlotState::new(SizeSpec::Percent(25)));
		slots.insert(
			DockSlot::Overlay,
			DockSlotState::new(SizeSpec::Percent(100)),
		);
		Self { slots }
	}

	pub fn open_panel(&mut self, slot: DockSlot, id: String) {
		let state = self
			.slots
			.entry(slot)
			.or_insert_with(|| DockSlotState::new(SizeSpec::Percent(30)));
		if !state.open.iter().any(|p| p == &id) {
			state.open.push(id.clone());
		}
		state.active = Some(id);
	}

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

	pub fn is_open(&self, id: &str) -> bool {
		self.slots.values().any(|s| s.open.iter().any(|p| p == id))
	}

	pub fn any_open(&self) -> bool {
		self.slots.values().any(|s| !s.open.is_empty())
	}

	pub fn active_in_slot(&self, slot: DockSlot) -> Option<&str> {
		self.slots.get(&slot).and_then(|s| s.active.as_deref())
	}

	pub fn compute_layout(&self, area: Rect) -> DockLayout {
		let mut layout = DockLayout {
			doc_area: area,
			..Default::default()
		};

		let has_top = self
			.slots
			.get(&DockSlot::Top)
			.map(|s| !s.open.is_empty())
			.unwrap_or(false);
		let has_bottom = self
			.slots
			.get(&DockSlot::Bottom)
			.map(|s| !s.open.is_empty())
			.unwrap_or(false);

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

		let has_left = self
			.slots
			.get(&DockSlot::Left)
			.map(|s| !s.open.is_empty())
			.unwrap_or(false);
		let has_right = self
			.slots
			.get(&DockSlot::Right)
			.map(|s| !s.open.is_empty())
			.unwrap_or(false);

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
