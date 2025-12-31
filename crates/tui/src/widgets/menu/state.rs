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

	/// Returns the current highlight depth (1 = top bar, 2 = first dropdown, etc.).
	fn depth(&self) -> usize {
		let mut depth = 0;
		let mut current = self.root.highlighted_child();
		while let Some(item) = current {
			depth += 1;
			current = item.highlighted_child();
		}
		depth
	}

	/// Moves highlight up in current dropdown, or closes dropdown if at top.
	pub fn up(&mut self) {
		match self.depth() {
			0 | 1 => {}
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

	/// Moves highlight down, or opens dropdown if on top bar.
	pub fn down(&mut self) {
		if self.depth() == 1 {
			self.push();
		} else {
			self.next();
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

	/// Moves highlight right (next top-level item, or opens submenu).
	pub fn right(&mut self) {
		match self.depth() {
			0 => {}
			1 => self.next(),
			2 => {
				if !self.push() {
					self.pop();
					self.next();
				}
			}
			_ => {
				self.push();
			}
		}
	}

	/// Selects the currently highlighted item.
	///
	/// If the item is a group, opens it. Otherwise, emits a [`MenuEvent::Selected`].
	pub fn select(&mut self) {
		if let Some(item) = self.root.highlight_mut() {
			if !item.children.is_empty() {
				self.push();
			} else if let Some(ref data) = item.data {
				self.events.push(MenuEvent::Selected(data.clone()));
			}
		}
	}

	/// Opens the submenu of the currently highlighted item.
	fn push(&mut self) -> bool {
		self.root
			.highlight_mut()
			.map(|item| item.highlight_first_child())
			.unwrap_or(false)
	}

	/// Closes the current submenu.
	fn pop(&mut self) {
		if let Some(item) = self.root.highlight_mut() {
			item.clear_highlight();
		}
	}

	/// Highlights the previous sibling.
	fn prev(&mut self) {
		if let Some(parent) = self.root.highlight_parent_mut() {
			parent.highlight_prev();
		} else {
			self.root.highlight_prev();
		}
	}

	/// Highlights the next sibling.
	fn next(&mut self) {
		if let Some(parent) = self.root.highlight_parent_mut() {
			parent.highlight_next();
		} else {
			self.root.highlight_next();
		}
	}

	/// Drains pending events. Call this each frame after rendering.
	pub fn drain_events(&mut self) -> impl Iterator<Item = MenuEvent<T>> + '_ {
		self.events.drain(..)
	}

	/// Returns the number of dropdown levels currently visible.
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

	fn bar_label_width(name: &str) -> u16 {
		UnicodeWidthStr::width(name) as u16 + 2
	}

	/// Handles a mouse click at the given position relative to menu area origin.
	///
	/// Returns true if the click was handled (hit a menu item).
	pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
		// Check menu bar (y == 0)
		if y == 0 {
			let mut current_x = 1u16; // Start after leading space
			for (idx, item) in self.root.children.iter().enumerate() {
				let label_width = Self::bar_label_width(item.name()); // " name "
				if x >= current_x && x < current_x.saturating_add(label_width) {
					// Clear existing highlights and highlight this item
					for child in &mut self.root.children {
						child.clear_highlight();
					}
					self.root.children[idx].highlighted = true;
					// Open dropdown
					self.push();
					return true;
				}
				current_x = current_x.saturating_add(label_width);
			}
			return false;
		}

		// Check first-level dropdown (y >= 1)
		let bar_idx = self
			.root
			.children
			.iter()
			.position(|c| c.highlighted)
			.unwrap_or(0);

		let items = &self.root.children.get(bar_idx).map(|c| &c.children);
		let Some(items) = items else {
			return false;
		};
		if items.is_empty() {
			return false;
		}

		let mut current_x = 1u16;
		let mut bar_start_x = current_x;
		for (idx, item) in self.root.children.iter().enumerate() {
			let label_width = Self::bar_label_width(item.name());
			if idx == bar_idx {
				bar_start_x = current_x;
				break;
			}
			current_x = current_x.saturating_add(label_width);
		}

		let max_name_width = items.iter().map(|i| i.name().len()).max().unwrap_or(0) as u16;
		let content_width = max_name_width + 4;

		// Dropdown items start at y=2 (y=1 is top padding)
		let item_y_start = 2u16;
		let item_y_end = item_y_start + items.len() as u16;
		// Items start 1 cell in from left padding
		let item_x_start = bar_start_x + 1;

		if y >= item_y_start
			&& y < item_y_end
			&& x >= item_x_start
			&& x < item_x_start.saturating_add(content_width)
		{
			let item_idx = (y - item_y_start) as usize;

			// Clear highlights and select item
			let bar_item = &mut self.root.children[bar_idx];
			for child in &mut bar_item.children {
				child.clear_highlight();
			}
			bar_item.children[item_idx].highlighted = true;

			// If group, open; otherwise emit event
			if bar_item.children[item_idx].is_group() {
				bar_item.children[item_idx].highlight_first_child();
			} else if let Some(ref data) = bar_item.children[item_idx].data {
				self.events.push(MenuEvent::Selected(data.clone()));
			}
			return true;
		}

		false
	}
}
