//! Menu state management.

use alloc::vec;
use alloc::vec::Vec;

use super::{MenuItem, MenuLayout};
use crate::layout::Position;

/// Events emitted by menu interactions.
#[derive(Debug)]
pub enum MenuEvent<T> {
	/// An item was selected.
	Selected(T),
}

/// Result of hit-testing a mouse position against menu regions.
enum HitResult {
	/// Mouse is over a bar item.
	Bar(usize),
	/// Mouse is over a dropdown item. Path from bar item (e.g., `[2]` for third
	/// item in first dropdown, `[1, 0]` for first item in nested submenu).
	Dropdown(Vec<usize>),
	/// Mouse is not over any menu region.
	Miss,
}

/// Runtime state for a menu bar.
///
/// Tracks the menu tree structure and current selection path.
pub struct MenuState<T> {
	/// Top-level menu items in the menu bar.
	pub(crate) items: Vec<MenuItem<T>>,
	/// Index path to currently selected item. Empty = inactive, `[0]` = first bar item.
	pub(crate) path: Vec<usize>,
	/// Whether dropdown is visible. Separate from path because a bar item can be
	/// highlighted without its dropdown open (e.g., after pressing up from dropdown).
	pub(crate) expanded: bool,
	/// Queue of events to be processed by the application.
	events: Vec<MenuEvent<T>>,
	/// Cached layout information for hit testing.
	layout: Option<MenuLayout>,
}

impl<T: Clone> MenuState<T> {
	/// Creates a new menu state with the given top-level items.
	pub fn new(items: Vec<MenuItem<T>>) -> Self {
		Self {
			items,
			path: Vec::new(),
			expanded: false,
			events: Vec::new(),
			layout: None,
		}
	}

	/// Activates the menu by highlighting the first top-level item.
	pub fn activate(&mut self) {
		if self.items.is_empty() {
			return;
		}
		self.path.clear();
		self.path.push(0);
		self.expanded = self.items[0].is_group();
	}

	/// Returns true if any menu item is currently highlighted.
	pub fn is_active(&self) -> bool {
		!self.path.is_empty()
	}

	/// Clears all highlights, deactivating the menu.
	pub fn reset(&mut self) {
		self.path.clear();
		self.expanded = false;
	}

	/// Returns the currently highlighted item.
	pub fn highlight(&self) -> Option<&MenuItem<T>> {
		self.selected_item()
	}

	/// Returns the currently selected item.
	pub fn selected_item(&self) -> Option<&MenuItem<T>> {
		self.item_at_path(&self.path)
	}

	/// Moves highlight up in current dropdown, or collapses if at top.
	pub fn up(&mut self) {
		match self.path.len() {
			0 => {}
			1 if self.expanded => {
				self.expanded = false;
			}
			n if n >= 2 => {
				let last = self.path.last_mut().expect("path length checked");
				if *last == 0 {
					self.path.pop();
				} else {
					*last -= 1;
				}
			}
			_ => {}
		}
	}

	/// Moves highlight down, or enters dropdown if on top bar.
	pub fn down(&mut self) {
		match self.path.len() {
			0 => {}
			1 if !self.expanded => {
				if self.bar_item().map(|item| item.is_group()).unwrap_or(false) {
					self.expanded = true;
				}
			}
			1 if self.expanded => {
				if self.bar_item().map(|item| item.is_group()).unwrap_or(false) {
					self.path.push(0);
				}
			}
			n if n >= 2 => {
				let len = self.sibling_len();
				if len == 0 {
					return;
				}
				let last = self.path.last_mut().expect("path length checked");
				*last = (*last + 1).min(len.saturating_sub(1));
			}
			_ => {}
		}
	}

	/// Moves highlight left (prev top-level item, or closes submenu).
	pub fn left(&mut self) {
		match self.path.len() {
			0 => {}
			1 => self.move_bar_prev(),
			2 => {
				self.path.truncate(1);
				self.move_bar_prev();
			}
			_ => {
				self.path.pop();
			}
		}
	}

	/// Moves highlight right (next top-level item, or enters submenu).
	pub fn right(&mut self) {
		match self.path.len() {
			0 => {}
			1 => self.move_bar_next(),
			2 => {
				let enter_submenu = self
					.selected_item()
					.map(|item| item.is_group())
					.unwrap_or(false);
				if enter_submenu {
					self.path.push(0);
				} else {
					self.path.truncate(1);
					self.move_bar_next();
				}
			}
			_ => {
				if self
					.selected_item()
					.map(|item| item.is_group())
					.unwrap_or(false)
				{
					self.path.push(0);
				}
			}
		}
	}

	/// Selects the currently highlighted item.
	///
	/// Groups are expanded with first child highlighted.
	/// Leaf items emit [`MenuEvent::Selected`].
	pub fn select(&mut self) {
		let (is_group, data) = match self.selected_item() {
			Some(item) => (item.is_group(), item.data.as_ref().cloned()),
			None => return,
		};

		if is_group {
			self.expanded = true;
			self.path.push(0);
			return;
		}

		if let Some(data) = data {
			self.events.push(MenuEvent::Selected(data));
		}
	}

	/// Drains pending events.
	pub fn drain_events(&mut self) -> impl Iterator<Item = MenuEvent<T>> + '_ {
		self.events.drain(..)
	}

	/// Returns the number of dropdown levels currently visible, used for
	/// reserving horizontal space when positioning near screen edge.
	pub(crate) fn dropdown_depth(&self) -> u16 {
		if !self.expanded || self.path.is_empty() {
			return 0;
		}
		let mut depth = 0u16;
		let mut items = self.items.as_slice();
		for &idx in &self.path {
			let Some(item) = items.get(idx) else {
				break;
			};
			if item.is_group() {
				depth += 1;
			}
			items = &item.children;
		}
		depth
	}

	/// Sets the cached layout for hit testing mouse interactions.
	pub(crate) fn set_layout(&mut self, layout: MenuLayout) {
		self.layout = Some(layout);
	}

	/// Handles a mouse click. Returns true if handled.
	pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
		match self.hit_test(x, y) {
			HitResult::Bar(idx) => {
				self.select_bar_item(idx);
				self.expanded = self.bar_item().map(|i| i.is_group()).unwrap_or(false);
				true
			}
			HitResult::Dropdown(path) => {
				self.set_dropdown_path(path);
				self.expanded = true;
				self.select();
				true
			}
			HitResult::Miss => false,
		}
	}

	/// Handles mouse hover. Returns true if over a menu item.
	pub fn handle_hover(&mut self, x: u16, y: u16) -> bool {
		match self.hit_test(x, y) {
			HitResult::Bar(idx) => {
				self.select_bar_item(idx);
				self.expanded = self.bar_item().map(|i| i.is_group()).unwrap_or(false);
				true
			}
			HitResult::Dropdown(path) => {
				self.set_dropdown_path(path);
				self.expanded = true;
				true
			}
			HitResult::Miss => false,
		}
	}

	/// Tests if a position hits any menu region.
	fn hit_test(&self, x: u16, y: u16) -> HitResult {
		let Some(layout) = &self.layout else {
			return HitResult::Miss;
		};
		let pos = Position { x, y };

		if let Some(idx) = layout.bar_regions.iter().position(|r| r.contains(pos)) {
			return HitResult::Bar(idx);
		}
		if let Some(dropdown) = &layout.dropdown {
			if let Some(path) = Self::hit_test_dropdown(dropdown, pos) {
				return HitResult::Dropdown(path);
			}
		}
		HitResult::Miss
	}

	/// Tests if a position hits any item in a dropdown menu.
	fn hit_test_dropdown(dropdown: &super::DropdownLayout, pos: Position) -> Option<Vec<usize>> {
		if let Some(submenu) = &dropdown.submenu {
			if let Some(mut path) = Self::hit_test_dropdown(submenu, pos) {
				let parent_idx = dropdown
					.item_regions
					.iter()
					.position(|r| r.y == submenu.area.y)?;
				path.insert(0, parent_idx);
				return Some(path);
			}
		}
		dropdown
			.item_regions
			.iter()
			.position(|r| r.contains(pos))
			.map(|idx| vec![idx])
	}

	/// Returns the item at the given index path.
	fn item_at_path(&self, path: &[usize]) -> Option<&MenuItem<T>> {
		let mut items = self.items.as_slice();
		let mut current = None;
		for &idx in path {
			let item = items.get(idx)?;
			current = Some(item);
			items = &item.children;
		}
		current
	}

	/// Returns the currently selected top-level bar item.
	fn bar_item(&self) -> Option<&MenuItem<T>> {
		self.path.first().and_then(|&idx| self.items.get(idx))
	}

	/// Returns the number of siblings at the current path level.
	fn sibling_len(&self) -> usize {
		if self.path.len() < 2 {
			return 0;
		}
		let parent_path = &self.path[..self.path.len().saturating_sub(1)];
		let Some(parent) = self.item_at_path(parent_path) else {
			return 0;
		};
		parent.children.len()
	}

	/// Moves selection to the previous bar item.
	fn move_bar_prev(&mut self) {
		let idx = self.path.first().copied().unwrap_or(0);
		self.select_bar_item(idx.saturating_sub(1));
	}

	/// Moves selection to the next bar item.
	fn move_bar_next(&mut self) {
		let idx = self.path.first().copied().unwrap_or(0);
		let max = self.items.len().saturating_sub(1);
		self.select_bar_item((idx + 1).min(max));
	}

	/// Selects a bar item by index.
	fn select_bar_item(&mut self, idx: usize) {
		if idx >= self.items.len() {
			return;
		}
		self.path.clear();
		self.path.push(idx);
	}

	/// Sets the dropdown path while preserving the bar selection.
	fn set_dropdown_path(&mut self, dropdown_path: Vec<usize>) {
		let bar_idx = self.path.first().copied().unwrap_or(0);
		self.path.clear();
		self.path.push(bar_idx);
		self.path.extend(dropdown_path);
	}
}
