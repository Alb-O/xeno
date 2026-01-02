//! Menu item tree node.

use alloc::borrow::Cow;
use alloc::vec::Vec;

/// A node in the menu tree.
///
/// Items can be leaf nodes (selectable, containing data) or groups (submenu containers).
pub struct MenuItem<T> {
	/// Display name for the menu item.
	pub(crate) name: Cow<'static, str>,
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
			data: Some(data),
			children: Vec::new(),
		}
	}

	/// Creates a group (submenu container).
	pub fn group(name: impl Into<Cow<'static, str>>, children: Vec<Self>) -> Self {
		Self {
			name: name.into(),
			data: None,
			children,
		}
	}

	/// Returns true if this item has children.
	pub fn is_group(&self) -> bool {
		!self.children.is_empty()
	}

	/// Returns the item's display name.
	pub fn name(&self) -> &str {
		&self.name
	}
}
