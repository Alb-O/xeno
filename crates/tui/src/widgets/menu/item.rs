//! Menu item tree node.

use alloc::borrow::Cow;
use alloc::string::ToString;
use alloc::vec::Vec;

/// Width of a nerd font icon in terminal cells (typically 2 for proper display).
pub const ICON_CELL_WIDTH: u16 = 2;

/// Padding after the icon before the label text.
pub const ICON_PADDING: u16 = 2;

/// Total width consumed by an icon: icon cells + padding.
pub const ICON_TOTAL_WIDTH: u16 = ICON_CELL_WIDTH + ICON_PADDING;

/// A node in the menu tree.
///
/// Items can be leaf nodes (selectable, containing data) or groups (submenu containers).
pub struct MenuItem<T> {
	/// Display name for the menu item.
	pub(crate) name: Cow<'static, str>,
	/// Optional icon (nerd font glyph) displayed before the name.
	pub(crate) icon: Option<Cow<'static, str>>,
	/// Associated data for leaf items, None for groups.
	pub(crate) data: Option<T>,
	/// Child items for submenu groups.
	pub(crate) children: Vec<MenuItem<T>>,
}

impl<T> MenuItem<T> {
	/// Creates a selectable leaf item.
	pub fn item(name: impl Into<Cow<'static, str>>, data: T) -> Self {
		Self {
			name: name.into(),
			icon: None,
			data: Some(data),
			children: Vec::new(),
		}
	}

	/// Creates a group (submenu container).
	pub fn group(name: impl Into<Cow<'static, str>>, children: Vec<Self>) -> Self {
		Self {
			name: name.into(),
			icon: None,
			data: None,
			children,
		}
	}

	/// Sets an icon for this menu item.
	///
	/// Icons are typically nerd font glyphs that display before the item name.
	/// They are rendered with [`ICON_CELL_WIDTH`] cells plus [`ICON_PADDING`].
	#[must_use]
	pub fn icon(mut self, icon: impl Into<Cow<'static, str>>) -> Self {
		self.icon = Some(icon.into());
		self
	}

	/// Sets an icon from a Unicode code point (hex value).
	///
	/// This is useful for specifying nerd font icons by their hex code
	/// without needing to copy-paste the actual glyph character.
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_tui::widgets::menu::MenuItem;
	///
	/// // Create menu item with folder icon (nf-fa-folder, U+F07B)
	/// let item = MenuItem::item("Open Folder", Action::OpenFolder)
	///     .icon_codepoint(0xF07B);
	///
	/// // Create menu item with save icon (nf-fa-floppy_o, U+F0C7)
	/// let item = MenuItem::item("Save", Action::Save)
	///     .icon_codepoint(0xF0C7);
	/// ```
	///
	/// If the code point is not a valid Unicode scalar value, no icon is set.
	#[must_use]
	pub fn icon_codepoint(mut self, codepoint: u32) -> Self {
		if let Some(c) = char::from_u32(codepoint) {
			self.icon = Some(Cow::Owned(c.to_string()));
		}
		self
	}

	/// Returns true if this item has children.
	pub fn is_group(&self) -> bool {
		!self.children.is_empty()
	}

	/// Returns the item's display name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Returns the item's icon, if any.
	pub fn get_icon(&self) -> Option<&str> {
		self.icon.as_deref()
	}
}
