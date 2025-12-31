//! Panel registration macro.
//!
//! The [`panel!`] macro registers panel types in the distributed slice.

/// Registers a panel type in the [`PANELS`](crate::panels::PANELS) slice.
///
/// Panels are toggleable split views (terminals, debug logs, file trees, etc.)
/// that integrate with the editor's layer system.
///
/// # Example
///
/// ```ignore
/// // Panel definition with inline factory
/// panel!(terminal, {
///     description: "Embedded terminal emulator",
///     mode_name: "TERMINAL",
///     layer: 1,
///     sticky: true,
///     factory: || Box::new(TerminalBuffer::new()),
/// });
///
/// // Panel definition without factory (factory registered elsewhere)
/// panel!(debug, {
///     description: "Debug log viewer",
///     mode_name: "DEBUG",
///     layer: 2,
/// });
/// ```
///
/// # Fields
///
/// - `description` (required): Human-readable description
/// - `mode_name` (required): Status bar mode text when focused (e.g., "DEBUG")
/// - `layer` (required): Layer index for docking (0 = base, higher overlays lower)
/// - `singleton` (optional): Only one instance allowed (default: true)
/// - `sticky` (optional): Resist losing focus on mouse hover (default: false)
/// - `priority` (optional): Priority within layer (default: 0)
/// - `factory` (optional): Factory function `fn() -> Box<dyn Any + Send>`
#[macro_export]
macro_rules! panel {
	($name:ident, {
		description: $desc:expr,
		mode_name: $mode_name:expr,
		layer: $layer:expr
		$(, singleton: $singleton:expr)?
		$(, sticky: $sticky:expr)?
		$(, priority: $priority:expr)?
		, factory: $factory:expr
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::panels::PANELS)]
			static [<PANEL_ $name:upper>]: $crate::panels::PanelDef = $crate::panels::PanelDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				description: $desc,
				mode_name: $mode_name,
				layer: $layer,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				singleton: $crate::__opt!($({$singleton})?, true),
				sticky: $crate::__opt!($({$sticky})?, false),
			};

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::panels::PANEL_FACTORIES)]
			static [<PANEL_FACTORY_ $name:upper>]: $crate::panels::PanelFactoryDef =
				$crate::panels::PanelFactoryDef {
					name: stringify!($name),
					factory: $factory,
				};
		}
	};

	($name:ident, {
		description: $desc:expr,
		mode_name: $mode_name:expr,
		layer: $layer:expr
		$(, singleton: $singleton:expr)?
		$(, sticky: $sticky:expr)?
		$(, priority: $priority:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::panels::PANELS)]
			static [<PANEL_ $name:upper>]: $crate::panels::PanelDef = $crate::panels::PanelDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				description: $desc,
				mode_name: $mode_name,
				layer: $layer,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				singleton: $crate::__opt!($({$singleton})?, true),
				sticky: $crate::__opt!($({$sticky})?, false),
			};
		}
	};
}
