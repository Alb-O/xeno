//! Menu widget rendering.

use alloc::format;
use alloc::vec::Vec;
use core::marker::PhantomData;

use super::{MenuItem, MenuState};
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
	default_style: Style,
	highlight_style: Style,
	dropdown_width: u16,
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

	fn render_dropdown(
		&self,
		x: u16,
		y: u16,
		items: &[MenuItem<T>],
		buf: &mut Buffer,
		remaining_depth: u16,
	) {
		let max_name_width = items
			.iter()
			.map(|item| item.name().len())
			.max()
			.unwrap_or(0) as u16;

		let content_width = max_name_width + 4;
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

		let mut active_submenu: Option<(u16, u16, &[MenuItem<T>])> = None;

		for (idx, item) in items.iter().enumerate() {
			let item_x = inner.x;
			let item_y = inner.y + idx as u16;

			if item_y >= inner.bottom() {
				break;
			}

			let mut label = format!(" {:<width$} ", item.name(), width = max_name_width as usize);
			if item.is_group() {
				label.pop();
				label.push('>');
			}

			let style = if item.highlighted {
				self.highlight_style
			} else {
				self.default_style
			};

			buf.set_span(item_x, item_y, &Span::styled(label, style), content_width);

			if item.highlighted && item.is_group() {
				active_submenu = Some((inner.right(), item_y, &item.children));
			}
		}

		if let Some((sub_x, sub_y, children)) = active_submenu {
			self.render_dropdown(sub_x, sub_y, children, buf, remaining_depth.saturating_sub(1));
		}
	}
}

impl<T: Clone> StatefulWidget for Menu<T> {
	type State = MenuState<T>;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		let area = area.clamp(*buf.area());
		let dropdown_depth = state.dropdown_depth();

		let mut spans = Vec::new();
		let mut x_pos = area.x;

		spans.push(Span::styled(" ", self.default_style));
		x_pos = x_pos.saturating_add(1);

		for item in &state.root.children {
			let style = if item.highlighted {
				self.highlight_style
			} else {
				self.default_style
			};

			let label = format!(" {} ", item.name());
			let span = Span::styled(label, style);
			let span_width = span.width() as u16;

			if item.is_group() && (item.is_expanded() || item.highlighted) {
				self.render_dropdown(x_pos, area.y + 1, &item.children, buf, dropdown_depth);
			}

			x_pos += span_width;
			spans.push(span);
		}

		buf.set_line(area.x, area.y, &Line::from(spans), area.width);
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
}
