//! A widget for displaying key bindings with tree-style connectors.
//!
//! [`KeyTree`] renders a root key with child continuations using
//! box-drawing characters.
//!
//! # Example
//!
//! ```
//! use xeno_tui::widgets::keytree::{KeyTree, KeyTreeNode};
//!
//! let children = vec![
//!     KeyTreeNode::new("g", "document_start"),
//!     KeyTreeNode::new("e", "document_end"),
//! ];
//! let tree = KeyTree::new("g", children);
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
	/// Optional suffix displayed after description with distinct style (e.g., "…").
	pub suffix: Option<Cow<'a, str>>,
}

impl<'a> KeyTreeNode<'a> {
	/// Creates a new node.
	pub fn new(key: impl Into<Cow<'a, str>>, description: impl Into<Cow<'a, str>>) -> Self {
		Self {
			key: key.into(),
			description: description.into(),
			suffix: None,
		}
	}

	/// Creates a new node with a suffix.
	pub fn with_suffix(
		key: impl Into<Cow<'a, str>>,
		description: impl Into<Cow<'a, str>>,
		suffix: impl Into<Cow<'a, str>>,
	) -> Self {
		Self {
			key: key.into(),
			description: description.into(),
			suffix: Some(suffix.into()),
		}
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
	/// Vertical line for separator.
	pub vertical: &'a str,
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
	vertical: line::VERTICAL,
};

/// Displays a root key with child continuations as a tree.
#[derive(Debug, Clone, Default)]
pub struct KeyTree<'a> {
	/// The root key label (e.g., the first key pressed like "ctrl-w").
	root: Cow<'a, str>,
	/// Optional description for root (e.g., "Window").
	root_desc: Option<Cow<'a, str>>,
	/// Ancestor nodes between root and children (intermediate keys in the sequence).
	ancestors: Vec<KeyTreeNode<'a>>,
	/// Child nodes representing available continuations.
	children: Vec<KeyTreeNode<'a>>,
	/// Symbols used for tree connectors.
	symbols: TreeSymbols<'a>,
	/// Style for already-pressed keys (root and ancestors).
	ancestor_style: Style,
	/// Style for child key labels (available options).
	key_style: Style,
	/// Style for descriptions.
	desc_style: Style,
	/// Style for suffix text (e.g., "…" indicator).
	suffix_style: Style,
	/// Style for tree connector lines.
	line_style: Style,
}

impl<'a> KeyTree<'a> {
	/// Creates a new key tree with a root key and its continuations.
	pub fn new(root: impl Into<Cow<'a, str>>, children: Vec<KeyTreeNode<'a>>) -> Self {
		Self {
			root: root.into(),
			children,
			..Default::default()
		}
	}

	/// Sets the description shown after the root key (e.g., "Window").
	#[must_use]
	pub fn root_desc(mut self, desc: impl Into<Cow<'a, str>>) -> Self {
		self.root_desc = Some(desc.into());
		self
	}

	/// Sets ancestor nodes between root and children.
	///
	/// These are rendered as a path from root to the current prefix level.
	#[must_use]
	pub fn ancestors(mut self, ancestors: Vec<KeyTreeNode<'a>>) -> Self {
		self.ancestors = ancestors;
		self
	}

	/// Sets the tree line symbols.
	#[must_use]
	pub const fn symbols(mut self, symbols: TreeSymbols<'a>) -> Self {
		self.symbols = symbols;
		self
	}

	/// Sets the style for already-pressed keys (root and ancestors).
	#[must_use]
	pub const fn ancestor_style(mut self, style: Style) -> Self {
		self.ancestor_style = style;
		self
	}

	/// Sets the style for child key labels (available options).
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

	/// Sets the style for suffix text (e.g., "…" indicator).
	#[must_use]
	pub const fn suffix_style(mut self, style: Style) -> Self {
		self.suffix_style = style;
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
		if area.is_empty() || self.children.is_empty() {
			return;
		}

		let mut y = area.y;
		let ancestor_indent = self.ancestors.len() as u16 * 2;

		let root_width = self.root.len().min(area.width as usize);
		buf.set_stringn(area.x, y, &self.root, root_width, self.ancestor_style);
		let mut x = area.x + root_width as u16 + 1;
		if let Some(ref desc) = self.root_desc
			&& x < area.right()
		{
			let desc_width = desc.len().min((area.right() - x) as usize);
			buf.set_stringn(x, y, desc, desc_width, self.desc_style);
			x += desc_width as u16;
		}
		if !self.ancestors.is_empty() && x < area.right() {
			buf.set_string(x, y, "…", self.suffix_style);
		}
		y += 1;

		let ancestor_count = self.ancestors.len();
		for (i, ancestor) in self.ancestors.iter().enumerate() {
			if y >= area.bottom() {
				return;
			}
			let has_children_below = i < ancestor_count - 1 || !self.children.is_empty();

			let indent = i as u16 * 2;
			let x = area.x + indent;

			if x < area.right() {
				buf.set_string(x, y, self.symbols.corner, self.line_style);
			}
			if x + 1 < area.right() {
				buf.set_string(x + 1, y, self.symbols.horizontal, self.line_style);
			}
			if x + 2 < area.right() {
				let key_width = ancestor.key.len().min((area.right() - x - 2) as usize);
				buf.set_stringn(x + 2, y, &ancestor.key, key_width, self.ancestor_style);
				let mut desc_x = x + 2 + key_width as u16 + 1;
				if desc_x < area.right() && !ancestor.description.is_empty() {
					let desc_width = ancestor
						.description
						.len()
						.min((area.right() - desc_x) as usize);
					buf.set_stringn(
						desc_x,
						y,
						&ancestor.description,
						desc_width,
						self.desc_style,
					);
					desc_x += desc_width as u16;
				}
				if has_children_below && desc_x < area.right() {
					buf.set_string(desc_x, y, "…", self.suffix_style);
				}
			}
			y += 1;
		}

		if y < area.bottom() {
			let x = area.x + ancestor_indent;
			if x < area.right() {
				buf.set_string(x, y, self.symbols.vertical, self.line_style);
			}
			y += 1;
		}

		for (i, node) in self.children.iter().enumerate() {
			if y >= area.bottom() {
				break;
			}

			let is_last = i == self.children.len() - 1;
			let connector = if is_last {
				self.symbols.corner
			} else {
				self.symbols.branch
			};

			let x = area.x + ancestor_indent;
			if x < area.right() {
				buf.set_string(x, y, connector, self.line_style);
			}

			let mut x = x + 1;
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
				x += desc_width as u16;
			}

			if let Some(ref suffix) = node.suffix
				&& x < area.right()
			{
				let suffix_width = suffix.len().min((area.right() - x) as usize);
				buf.set_stringn(x, y, suffix, suffix_width, self.suffix_style);
			}

			y += 1;
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
		let tree = KeyTree::new("g", vec![]);
		let lines = render_to_lines(tree, 20, 5);
		assert!(lines.iter().all(|l| l.is_empty()));
	}

	#[test]
	fn root_with_single_child() {
		let children = vec![KeyTreeNode::new("g", "document_start")];
		let tree = KeyTree::new("g", children);
		let lines = render_to_lines(tree, 25, 4);
		assert_eq!(lines[0], "g");
		assert_eq!(lines[1], "│");
		assert!(lines[2].contains("╰─g document_start"));
	}

	#[test]
	fn root_with_multiple_children() {
		let children = vec![
			KeyTreeNode::new("g", "start"),
			KeyTreeNode::new("e", "end"),
			KeyTreeNode::new("h", "home"),
		];
		let tree = KeyTree::new("g", children);
		let lines = render_to_lines(tree, 20, 6);
		assert_eq!(lines[0], "g");
		assert_eq!(lines[1], "│");
		assert!(lines[2].contains("├─g"));
		assert!(lines[3].contains("├─e"));
		assert!(lines[4].contains("╰─h"));
	}

	#[test]
	fn truncates_to_area() {
		use unicode_width::UnicodeWidthStr;
		let children = vec![KeyTreeNode::new("g", "a very long description")];
		let tree = KeyTree::new("g", children);
		let lines = render_to_lines(tree, 12, 4);
		assert!(lines[2].width() <= 12);
	}

	#[test]
	fn renders_with_ancestors() {
		let ancestors = vec![KeyTreeNode::new("b", "Buffer")];
		let children = vec![
			KeyTreeNode::new("n", "Next"),
			KeyTreeNode::new("p", "Previous"),
		];
		let tree = KeyTree::new("ctrl-w", children)
			.root_desc("Window")
			.ancestors(ancestors);
		let lines = render_to_lines(tree, 25, 7);
		assert!(lines[0].contains("ctrl-w"));
		assert!(lines[0].contains("Window"));
		assert!(lines[1].contains("╰─b Buffer"));
		assert!(lines[2].contains("│"));
		assert!(lines[3].contains("├─n Next"));
		assert!(lines[4].contains("╰─p Previous"));
	}
}
