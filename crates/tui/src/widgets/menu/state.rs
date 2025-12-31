//! Menu state management.

use alloc::vec::Vec;

use unicode_width::UnicodeWidthStr;

use super::MenuItem;

/// Events emitted by menu interactions.
#[derive(Debug)]
pub enum MenuEvent<T> {
	/// An item was selected.
	Selected(T),
}

/// Runtime state for a menu bar.
///
/// Tracks the menu tree structure and current highlight/selection state.
pub struct MenuState<T> {
	pub(crate) root: MenuItem<T>,
	events: Vec<MenuEvent<T>>,
}

impl<T: Clone> MenuState<T> {
	/// Creates a new menu state with the given top-level items.
	pub fn new(items: Vec<MenuItem<T>>) -> Self {
		let mut root = MenuItem::group("root", items);
		root.highlighted = true;
		Self {
			root,
			events: Vec::new(),
		}
	}

	/// Activates the menu by highlighting the first top-level item.
	pub fn activate(&mut self) {
		self.root.highlight_first_child();
	}

	/// Returns true if any menu item is currently highlighted.
	pub fn is_active(&self) -> bool {
		self.root.highlighted_child().is_some()
	}

	/// Clears all highlights, deactivating the menu.
	pub fn reset(&mut self) {
		for child in &mut self.root.children {
			child.clear_highlight();
		}
	}

	/// Returns the currently highlighted item.
	pub fn highlight(&self) -> Option<&MenuItem<T>> {
		self.root.highlight()
	}

	fn depth(&self) -> usize {
		let mut depth = 0;
		let mut current = self.root.highlighted_child();
		while let Some(item) = current {
			depth += 1;
			current = item.highlighted_child();
		}
		depth
	}

	/// Moves highlight up in current dropdown, or collapses if at top.
	pub fn up(&mut self) {
		match self.depth() {
			0 | 1 => {
				if let Some(item) = self.root.highlighted_child_mut() {
					if item.is_expanded() {
						item.collapse();
					}
				}
			}
			2 => {
				if self
					.root
					.highlighted_child()
					.and_then(|c| c.highlighted_child_index())
					== Some(0)
				{
					self.pop();
				} else {
					self.prev();
				}
			}
			_ => self.prev(),
		}
	}

	/// Moves highlight down, or enters dropdown if on top bar.
	pub fn down(&mut self) {
		match self.depth() {
			1 => {
				if let Some(item) = self.root.highlighted_child_mut() {
					item.expand();
					item.highlight_first_child();
				}
			}
			_ => self.next(),
		}
	}

	/// Moves highlight left (prev top-level item, or closes submenu).
	pub fn left(&mut self) {
		match self.depth() {
			0 => {}
			1 => self.prev(),
			2 => {
				self.pop();
				self.prev();
			}
			_ => self.pop(),
		}
	}

	/// Moves highlight right (next top-level item, or enters submenu).
	pub fn right(&mut self) {
		match self.depth() {
			0 => {}
			1 => self.next(),
			2 => {
				if let Some(item) = self.root.highlight_mut() {
					if item.is_group() {
						item.expand();
						item.highlight_first_child();
						return;
					}
				}
				self.pop();
				self.next();
			}
			_ => {
				if let Some(item) = self.root.highlight_mut() {
					if item.is_group() {
						item.expand();
						item.highlight_first_child();
					}
				}
			}
		}
	}

	/// Selects the currently highlighted item.
	///
	/// Groups are expanded with first child highlighted.
	/// Leaf items emit [`MenuEvent::Selected`].
	pub fn select(&mut self) {
		if let Some(item) = self.root.highlight_mut() {
			if !item.children.is_empty() {
				item.expand();
				item.highlight_first_child();
			} else if let Some(ref data) = item.data {
				self.events.push(MenuEvent::Selected(data.clone()));
			}
		}
	}

	fn expand_current(&mut self) -> bool {
		if let Some(item) = self.root.highlight_mut() {
			if item.is_group() {
				item.expand();
				return true;
			}
		}
		false
	}

	fn pop(&mut self) {
		if let Some(item) = self.root.highlight_mut() {
			item.clear_highlight();
		}
	}

	fn prev(&mut self) {
		if let Some(parent) = self.root.highlight_parent_mut() {
			parent.highlight_prev();
		} else {
			self.root.highlight_prev();
		}
	}

	fn next(&mut self) {
		if let Some(parent) = self.root.highlight_parent_mut() {
			parent.highlight_next();
		} else {
			self.root.highlight_next();
		}
	}

	/// Drains pending events.
	pub fn drain_events(&mut self) -> impl Iterator<Item = MenuEvent<T>> + '_ {
		self.events.drain(..)
	}

	pub(crate) fn dropdown_depth(&self) -> u16 {
		let mut node = &self.root;
		let mut count = 0;
		while let Some(child) = node.highlighted_child() {
			if child.is_group() || node.children.iter().any(|c| c.is_group()) {
				count += 1;
			}
			node = child;
		}
		count
	}

	fn bar_item_x(&self, target_idx: usize) -> u16 {
		let mut x = 1u16;
		for (idx, item) in self.root.children.iter().enumerate() {
			if idx == target_idx {
				break;
			}
			x = x.saturating_add(UnicodeWidthStr::width(item.name()) as u16 + 2);
		}
		x
	}

	fn bar_item_at(&self, x: u16) -> Option<usize> {
		let mut current_x = 1u16;
		for (idx, item) in self.root.children.iter().enumerate() {
			let width = UnicodeWidthStr::width(item.name()) as u16 + 2;
			if x >= current_x && x < current_x.saturating_add(width) {
				return Some(idx);
			}
			current_x = current_x.saturating_add(width);
		}
		None
	}

	/// Handles a mouse click. Returns true if handled.
	pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
		if y == 0 {
			if let Some(idx) = self.bar_item_at(x) {
				for child in &mut self.root.children {
					child.clear_highlight();
				}
				self.root.children[idx].highlighted = true;
				self.expand_current();
				return true;
			}
			return false;
		}

		let Some(bar_idx) = self.root.children.iter().position(|c| c.highlighted) else {
			return false;
		};

		let children = &self.root.children[bar_idx].children;
		if children.is_empty() {
			return false;
		}

		let bar_x = self.bar_item_x(bar_idx);
		let max_width = children.iter().map(|i| i.name().len()).max().unwrap_or(0) as u16;
		let content_width = max_width + 4;

		let item_y_start = 2u16;
		let item_x_start = bar_x + 1;

		if y >= item_y_start
			&& y < item_y_start + children.len() as u16
			&& x >= item_x_start
			&& x < item_x_start.saturating_add(content_width)
		{
			let item_idx = (y - item_y_start) as usize;

			let bar_item = &mut self.root.children[bar_idx];
			for child in &mut bar_item.children {
				child.clear_highlight();
			}
			bar_item.children[item_idx].highlighted = true;

			if bar_item.children[item_idx].is_group() {
				bar_item.children[item_idx].highlight_first_child();
			} else if let Some(ref data) = bar_item.children[item_idx].data {
				self.events.push(MenuEvent::Selected(data.clone()));
			}
			return true;
		}

		false
	}

	/// Handles mouse hover. Returns true if over a menu item.
	pub fn handle_hover(&mut self, x: u16, y: u16) -> bool {
		if y == 0 {
			if let Some(idx) = self.bar_item_at(x) {
				if !self.root.children[idx].highlighted {
					for child in &mut self.root.children {
						child.clear_highlight();
					}
					self.root.children[idx].highlighted = true;
					self.expand_current();
				}
				return true;
			}
			return false;
		}

		let Some(bar_idx) = self.root.children.iter().position(|c| c.highlighted) else {
			return false;
		};

		let items_len = self.root.children[bar_idx].children.len();
		if items_len == 0 {
			return false;
		}

		let bar_x = self.bar_item_x(bar_idx);
		let max_width = self.root.children[bar_idx]
			.children
			.iter()
			.map(|i| i.name().len())
			.max()
			.unwrap_or(0) as u16;
		let content_width = max_width + 4;

		let item_y_start = 2u16;
		let item_x_start = bar_x + 1;

		if y >= item_y_start
			&& y < item_y_start + items_len as u16
			&& x >= item_x_start
			&& x < item_x_start.saturating_add(content_width)
		{
			let item_idx = (y - item_y_start) as usize;

			if !self.root.children[bar_idx].children[item_idx].highlighted {
				let bar_item = &mut self.root.children[bar_idx];
				for child in &mut bar_item.children {
					child.clear_highlight();
				}
				bar_item.children[item_idx].highlighted = true;
			}
			return true;
		}

		false
	}
}
