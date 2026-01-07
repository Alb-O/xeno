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

/// Registers a menu group in [`MENU_GROUPS`].
#[macro_export]
macro_rules! menu_group {
	($name:ident, {
		label: $label:expr
		$(, icon: $icon:expr)?
		$(, priority: $priority:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::MENU_GROUPS)]
			static [<MENU_GROUP_ $name>]: $crate::MenuGroupDef = $crate::MenuGroupDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				label: $label,
				icon: $crate::__menu_opt!($({Some($icon)})?, None),
				priority: $crate::__menu_opt!($({$priority})?, 50),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Registers a menu item in [`MENU_ITEMS`].
#[macro_export]
macro_rules! menu_item {
	($name:ident, {
		group: $group:expr,
		label: $label:expr,
		command: $command:expr
		$(, icon: $icon:expr)?
		$(, shortcut: $shortcut:expr)?
		$(, priority: $priority:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::MENU_ITEMS)]
			static [<MENU_ITEM_ $name>]: $crate::MenuItemDef = $crate::MenuItemDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				group: $group,
				label: $label,
				command: $command,
				icon: $crate::__menu_opt!($({Some($icon)})?, None),
				shortcut: $crate::__menu_opt!($({Some($shortcut)})?, None),
				priority: $crate::__menu_opt!($({$priority})?, 50),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}
