//! Menu type definitions and distributed slices.

use linkme::distributed_slice;

use crate::RegistrySource;

/// Definition of a top-level menu group (e.g., "File", "Edit").
pub struct MenuGroupDef {
	/// Fully qualified ID: "crate_name::group_name".
	pub id: &'static str,
	/// Group identifier for item matching.
	pub name: &'static str,
	/// Display label in menu bar.
	pub label: &'static str,
	/// Ordering priority (lower = leftmost).
	pub priority: i16,
	/// Source crate.
	pub source: RegistrySource,
}

/// Definition of a menu item within a group.
pub struct MenuItemDef {
	/// Fully qualified ID: "crate_name::item_name".
	pub id: &'static str,
	/// Item identifier.
	pub name: &'static str,
	/// Parent group name (matches MenuGroupDef.name).
	pub group: &'static str,
	/// Display label in dropdown.
	pub label: &'static str,
	/// Command to execute when selected.
	pub command: &'static str,
	/// Optional keyboard shortcut hint (display only).
	pub shortcut: Option<&'static str>,
	/// Ordering priority within group (lower = higher in menu).
	pub priority: i16,
	/// Source crate.
	pub source: RegistrySource,
}

/// Registry of all menu group definitions.
#[distributed_slice]
pub static MENU_GROUPS: [MenuGroupDef];

/// Registry of all menu item definitions.
#[distributed_slice]
pub static MENU_ITEMS: [MenuItemDef];

/// Returns all registered menu groups.
pub fn all_groups() -> &'static [MenuGroupDef] {
	&MENU_GROUPS
}

/// Returns all menu items for a given group name.
pub fn items_for_group(group_name: &str) -> impl Iterator<Item = &'static MenuItemDef> + '_ {
	MENU_ITEMS
		.iter()
		.filter(move |item| item.group == group_name)
}
