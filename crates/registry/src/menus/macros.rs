//! Menu registration macros.

/// Helper macro for optional values with defaults.
#[doc(hidden)]
#[macro_export]
macro_rules! __menu_opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Registers a menu group in the [`MENU_GROUPS`](crate::menus::MENU_GROUPS) slice.
///
/// # Example
///
/// ```ignore
/// menu_group!(file, {
///     label: "File",
///     priority: 0,
/// });
/// ```
#[macro_export]
macro_rules! menu_group {
	($name:ident, {
		label: $label:expr
		$(, priority: $priority:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::menus::MENU_GROUPS)]
			static [<MENU_GROUP_ $name>]: $crate::menus::MenuGroupDef = $crate::menus::MenuGroupDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				label: $label,
				priority: $crate::__menu_opt!($({$priority})?, 50),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Registers a menu item in the [`MENU_ITEMS`](crate::menus::MENU_ITEMS) slice.
///
/// # Example
///
/// ```ignore
/// menu_item!(file_save, {
///     group: "file",
///     label: "Save",
///     command: "write",
///     shortcut: "Ctrl+S",  // optional
///     priority: 20,
/// });
/// ```
#[macro_export]
macro_rules! menu_item {
	($name:ident, {
		group: $group:expr,
		label: $label:expr,
		command: $command:expr
		$(, shortcut: $shortcut:expr)?
		$(, priority: $priority:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::menus::MENU_ITEMS)]
			static [<MENU_ITEM_ $name>]: $crate::menus::MenuItemDef = $crate::menus::MenuItemDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				group: $group,
				label: $label,
				command: $command,
				shortcut: $crate::__menu_opt!($({Some($shortcut)})?, None),
				priority: $crate::__menu_opt!($({$priority})?, 50),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}
