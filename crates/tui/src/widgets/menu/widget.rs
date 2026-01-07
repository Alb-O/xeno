//! Menu widget rendering.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::marker::PhantomData;

use super::item::ICON_TOTAL_WIDTH;
use super::{DropdownLayout, MenuItem, MenuLayout, MenuState};
use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Color, Style};
use crate::text::{Line, Span};
use crate::widgets::block::Block;
use crate::widgets::clear::Clear;
use crate::widgets::{StatefulWidget, Widget};

/// A horizontal menu bar with dropdown submenus.
///
/// Renders as a single-line bar. Highlighted items show their dropdowns below,
/// overlaying other content.
pub struct Menu<T> {
	/// Style for non-highlighted menu items.
	default_style: Style,
	/// Style for the currently highlighted item.
	highlight_style: Style,
	/// Width of dropdown submenus in characters.
	dropdown_width: u16,
	/// Phantom data to hold the generic type.
	_marker: PhantomData<T>,
}

impl<T> Default for Menu<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> Menu<T> {
	/// Creates a new menu with default styling.
	pub fn new() -> Self {
		Self {
			default_style: Style::default().fg(Color::White),
			highlight_style: Style::default().fg(Color::White).bg(Color::LightBlue),
			dropdown_width: 20,
			_marker: PhantomData,
		}
	}

	/// Sets the default (non-highlighted) style.
	pub fn style(mut self, style: Style) -> Self {
		self.default_style = style;
		self
	}

	/// Sets the highlighted item style.
	pub fn highlight_style(mut self, style: Style) -> Self {
		self.highlight_style = style;
		self
	}

	/// Sets the minimum dropdown width.
	pub fn dropdown_width(mut self, width: u16) -> Self {
		self.dropdown_width = width;
		self
	}

	/// Renders a dropdown menu at the given position.
	fn render_dropdown(
		&self,
		x: u16,
		y: u16,
		items: &[MenuItem<T>],
		buf: &mut Buffer,
		remaining_depth: u16,
		highlight_path: &[usize],
	) -> DropdownLayout {
		let max_name_width = items
			.iter()
			.map(|item| item.name().len())
			.max()
			.unwrap_or(0) as u16;

		let has_icons = items.iter().any(|item| item.get_icon().is_some());
		let icon_column_width = if has_icons { ICON_TOTAL_WIDTH } else { 0 };
		let content_width = 1 + icon_column_width + max_name_width;
		let block = Block::bordered().style(self.default_style);
		let width = content_width + 2;
		let height = items.len() as u16 + 2;

		let space_needed = remaining_depth * self.dropdown_width;
		let max_x = buf.area().right().saturating_sub(space_needed);
		let x = x.min(max_x);

		let area = Rect::new(x, y, width, height).clamp(*buf.area());

		Clear.render(area, buf);
		let inner = block.inner(area);
		block.render(area, buf);

		let mut submenu: Option<Box<DropdownLayout>> = None;
		let mut item_regions = Vec::new();
		let highlighted = highlight_path.first().copied();

		for (idx, item) in items.iter().enumerate() {
			let item_x = inner.x;
			let item_y = inner.y + idx as u16;

			if item_y >= inner.bottom() {
				break;
			}

			let mut label = if has_icons {
				let icon_pad = " ".repeat(super::item::ICON_PADDING as usize);
				match item.get_icon() {
					Some(icon) => format!(
						" {}{}{:<width$} ",
						icon,
						icon_pad,
						item.name(),
						width = max_name_width as usize
					),
					None => format!(
						" {}{:<width$} ",
						" ".repeat(ICON_TOTAL_WIDTH as usize),
						item.name(),
						width = max_name_width as usize
					),
				}
			} else {
				format!(" {:<width$} ", item.name(), width = max_name_width as usize)
			};
			if item.is_group() {
				label.pop();
				label.push('>');
			}

			let is_highlighted = highlighted == Some(idx);
			let style = if is_highlighted {
				self.highlight_style
			} else {
				self.default_style
			};

			buf.set_span(item_x, item_y, &Span::styled(label, style), content_width);
			item_regions.push(Rect::new(item_x, item_y, content_width, 1));

			if is_highlighted && item.is_group() {
				let sub_path = highlight_path.get(1..).unwrap_or(&[]);
				let sub_layout = self.render_dropdown(
					inner.right(),
					item_y,
					&item.children,
					buf,
					remaining_depth.saturating_sub(1),
					sub_path,
				);
				submenu = Some(Box::new(sub_layout));
			}
		}

		DropdownLayout {
			area,
			item_regions,
			submenu,
		}
	}
}

impl<T: Clone> StatefulWidget for Menu<T> {
	type State = MenuState<T>;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		let area = area.clamp(*buf.area());
		let dropdown_depth = state.dropdown_depth();
		let active_bar = state.path.first().copied();

		let mut spans = Vec::new();
		let mut x_pos = area.x;
		let mut layout = MenuLayout {
			bar_regions: Vec::new(),
			dropdown: None,
		};

		spans.push(Span::styled(" ", self.default_style));
		x_pos = x_pos.saturating_add(1);

		for (idx, item) in state.items.iter().enumerate() {
			let is_selected = active_bar == Some(idx);
			let style = if is_selected {
				self.highlight_style
			} else {
				self.default_style
			};

			let label = match item.get_icon() {
				Some(icon) => {
					let icon_pad = " ".repeat(super::item::ICON_PADDING as usize);
					format!(" {}{}{} ", icon, icon_pad, item.name())
				}
				None => format!(" {} ", item.name()),
			};
			let span = Span::styled(label, style);
			let span_width = span.width() as u16;
			layout
				.bar_regions
				.push(Rect::new(x_pos, area.y, span_width, 1));

			if is_selected && state.expanded && item.is_group() {
				let highlight_path = state.path.get(1..).unwrap_or(&[]);
				layout.dropdown = Some(self.render_dropdown(
					x_pos,
					area.y + 1,
					&item.children,
					buf,
					dropdown_depth,
					highlight_path,
				));
			}

			x_pos = x_pos.saturating_add(span_width);
			spans.push(span);
		}

		buf.set_line(area.x, area.y, &Line::from(spans), area.width);
		state.set_layout(layout);
	}
}

#[cfg(test)]
mod tests {
	use alloc::vec;

	use super::*;

	#[test]
	fn menu_state_navigation() {
		let mut state: MenuState<&str> = MenuState::new(vec![
			MenuItem::group(
				"File",
				vec![
					MenuItem::item("New", "file:new"),
					MenuItem::item("Open", "file:open"),
				],
			),
			MenuItem::group("Edit", vec![MenuItem::item("Undo", "edit:undo")]),
		]);

		assert!(!state.is_active());

		state.activate();
		assert!(state.is_active());
		assert_eq!(state.highlight().unwrap().name(), "File");

		state.right();
		assert_eq!(state.highlight().unwrap().name(), "Edit");

		state.down();
		assert_eq!(state.highlight().unwrap().name(), "Undo");

		state.select();
		let events: Vec<_> = state.drain_events().collect();
		assert_eq!(events.len(), 1);
	}

	#[test]
	fn nested_submenu_navigation() {
		let mut state: MenuState<&str> = MenuState::new(vec![MenuItem::group(
			"File",
			vec![
				MenuItem::item("New", "file:new"),
				MenuItem::group(
					"Recent",
					vec![
						MenuItem::item("doc1.txt", "recent:1"),
						MenuItem::item("doc2.txt", "recent:2"),
					],
				),
			],
		)]);

		state.activate();
		assert_eq!(state.path, vec![0]);

		state.down();
		assert_eq!(state.path, vec![0, 0]);
		assert_eq!(state.highlight().unwrap().name(), "New");

		state.down();
		assert_eq!(state.path, vec![0, 1]);
		assert_eq!(state.highlight().unwrap().name(), "Recent");

		state.right();
		assert_eq!(state.path, vec![0, 1, 0]);
		assert_eq!(state.highlight().unwrap().name(), "doc1.txt");

		state.down();
		assert_eq!(state.path, vec![0, 1, 1]);
		assert_eq!(state.highlight().unwrap().name(), "doc2.txt");

		state.left();
		assert_eq!(state.path, vec![0, 1]);
		assert_eq!(state.highlight().unwrap().name(), "Recent");

		// Exit to bar with another left
		state.left();
		assert_eq!(state.path, vec![0]);
	}
}
