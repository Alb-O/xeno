//! Menu definition types and registry.
//!
//! This module defines the structures for menu groups and items,
//! along with the distributed slices for compile-time registration.

use linkme::distributed_slice;

use crate::{RegistrySource, impl_registry_metadata};

/// A top-level menu group (e.g., "File", "Edit").
pub struct MenuGroupDef {
	/// Unique identifier for this menu group.
	pub id: &'static str,
	/// Display name for the menu group.
	pub name: &'static str,
	/// Label shown in the menu bar (may include accelerator key).
	pub label: &'static str,
	/// Sort priority for ordering groups in the menu bar.
	pub priority: i16,
	/// Registry source indicating where this group was registered.
	pub source: RegistrySource,
}

/// A menu item within a group.
pub struct MenuItemDef {
	/// Unique identifier for this menu item.
	pub id: &'static str,
	/// Display name for the menu item.
	pub name: &'static str,
	/// ID of the parent menu group.
	pub group: &'static str,
	/// Label shown in the dropdown menu.
	pub label: &'static str,
	/// Command to execute when the item is selected.
	pub command: &'static str,
	/// Optional keyboard shortcut displayed next to the label.
	pub shortcut: Option<&'static str>,
	/// Sort priority for ordering items within the group.
	pub priority: i16,
	/// Registry source indicating where this item was registered.
	pub source: RegistrySource,
}

impl_registry_metadata!(MenuGroupDef);
impl_registry_metadata!(MenuItemDef);

/// Distributed slice containing all registered menu groups.
#[distributed_slice]
pub static MENU_GROUPS: [MenuGroupDef];

/// Distributed slice containing all registered menu items.
#[distributed_slice]
pub static MENU_ITEMS: [MenuItemDef];

/// Returns all registered menu groups.
pub fn all_groups() -> &'static [MenuGroupDef] {
	&MENU_GROUPS
}

/// Returns menu items for a given group name.
pub fn items_for_group(group_name: &str) -> impl Iterator<Item = &'static MenuItemDef> + '_ {
	MENU_ITEMS
		.iter()
		.filter(move |item| item.group == group_name)
}
