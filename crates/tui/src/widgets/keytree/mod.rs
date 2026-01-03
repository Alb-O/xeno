//! A widget for displaying key bindings with tree-style connectors.
//!
//! [`KeyTree`] renders a flat list of key→description pairs with
//! box-drawing characters indicating structure.
//!
//! # Example
//!
//! ```
//! use evildoer_tui::widgets::keytree::{KeyTree, KeyTreeNode};
//!
//! let nodes = vec![
//!     KeyTreeNode::new("g", "goto..."),
//!     KeyTreeNode::new("z", "view..."),
//! ];
//! let tree = KeyTree::new(nodes);
//! ```

use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;
use crate::symbols::line;
use crate::widgets::Widget;

/// A key binding entry with its description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTreeNode<'a> {
	/// The key (e.g., "g", "space").
	pub key: Cow<'a, str>,
	/// Description of the action.
	pub description: Cow<'a, str>,
}

impl<'a> KeyTreeNode<'a> {
	/// Creates a new node.
	pub fn new(key: impl Into<Cow<'a, str>>, description: impl Into<Cow<'a, str>>) -> Self {
		Self { key: key.into(), description: description.into() }
	}
}

/// Line symbols for tree connectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSymbols<'a> {
	/// Branch connector (middle items).
	pub branch: &'a str,
	/// Corner connector (last item).
	pub corner: &'a str,
	/// Horizontal line after connector.
	pub horizontal: &'a str,
}

impl Default for TreeSymbols<'_> {
	fn default() -> Self {
		ROUNDED_SYMBOLS
	}
}

/// Rounded tree symbols (default).
pub const ROUNDED_SYMBOLS: TreeSymbols<'static> = TreeSymbols {
	branch: line::VERTICAL_RIGHT,
	corner: line::ROUNDED_BOTTOM_LEFT,
	horizontal: line::HORIZONTAL,
};

/// Displays key bindings as a list with tree connectors.
#[derive(Debug, Clone, Default)]
pub struct KeyTree<'a> {
	nodes: Vec<KeyTreeNode<'a>>,
	symbols: TreeSymbols<'a>,
	key_style: Style,
	desc_style: Style,
	line_style: Style,
}

impl<'a> KeyTree<'a> {
	/// Creates a new key tree with the given nodes.
	pub fn new(nodes: Vec<KeyTreeNode<'a>>) -> Self {
		Self { nodes, ..Default::default() }
	}

	/// Sets the tree line symbols.
	#[must_use]
	pub const fn symbols(mut self, symbols: TreeSymbols<'a>) -> Self {
		self.symbols = symbols;
		self
	}

	/// Sets the style for key labels.
	#[must_use]
	pub const fn key_style(mut self, style: Style) -> Self {
		self.key_style = style;
		self
	}

	/// Sets the style for descriptions.
	#[must_use]
	pub const fn desc_style(mut self, style: Style) -> Self {
		self.desc_style = style;
		self
	}

	/// Sets the style for tree connector lines.
	#[must_use]
	pub const fn line_style(mut self, style: Style) -> Self {
		self.line_style = style;
		self
	}
}

impl Widget for KeyTree<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		if area.is_empty() || self.nodes.is_empty() {
			return;
		}

		for (i, node) in self.nodes.iter().enumerate() {
			let y = area.y + i as u16;
			if y >= area.bottom() {
				break;
			}

			let is_last = i == self.nodes.len() - 1;
			let connector = if is_last { self.symbols.corner } else { self.symbols.branch };

			let mut x = area.x;
			buf.set_string(x, y, connector, self.line_style);
			x += 1;

			if x < area.right() {
				buf.set_string(x, y, self.symbols.horizontal, self.line_style);
				x += 1;
			}

			if x < area.right() {
				let key_width = node.key.len().min((area.right() - x) as usize);
				buf.set_stringn(x, y, &node.key, key_width, self.key_style);
				x += key_width as u16;
			}

			if x < area.right() {
				buf.set_string(x, y, " ", self.desc_style);
				x += 1;
			}

			if x < area.right() {
				let desc_width = node.description.len().min((area.right() - x) as usize);
				buf.set_stringn(x, y, &node.description, desc_width, self.desc_style);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use alloc::string::{String, ToString};
	use alloc::vec;

	use super::*;

	fn render_to_lines(tree: KeyTree<'_>, width: u16, height: u16) -> Vec<String> {
		let area = Rect::new(0, 0, width, height);
		let mut buf = Buffer::empty(area);
		tree.render(area, &mut buf);

		(0..height)
			.map(|y| {
				(0..width)
					.map(|x| buf[(x, y)].symbol().to_string())
					.collect::<String>()
					.trim_end()
					.to_string()
			})
			.collect()
	}

	#[test]
	fn empty_tree_renders_nothing() {
		let tree = KeyTree::default();
		let lines = render_to_lines(tree, 20, 5);
		assert!(lines.iter().all(|l| l.is_empty()));
	}

	#[test]
	fn single_node() {
		let nodes = vec![KeyTreeNode::new("g", "goto mode")];
		let tree = KeyTree::new(nodes);
		let lines = render_to_lines(tree, 20, 3);
		assert!(lines[0].contains("╰"));
		assert!(lines[0].contains("g goto mode"));
	}

	#[test]
	fn multiple_nodes_show_connectors() {
		let nodes = vec![
			KeyTreeNode::new("g", "goto"),
			KeyTreeNode::new("z", "view"),
			KeyTreeNode::new("m", "match"),
		];
		let tree = KeyTree::new(nodes);
		let lines = render_to_lines(tree, 20, 5);

		assert!(lines[0].contains("├"));
		assert!(lines[1].contains("├"));
		assert!(lines[2].contains("╰"));
	}

	#[test]
	fn truncates_to_area() {
		use unicode_width::UnicodeWidthStr;
		let nodes = vec![KeyTreeNode::new("g", "a very long description")];
		let tree = KeyTree::new(nodes);
		let lines = render_to_lines(tree, 12, 1);
		assert!(lines[0].width() <= 12);
	}
}
