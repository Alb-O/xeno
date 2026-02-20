//! Registration macros for gutter columns.

/// Registers a handler for a registry-defined gutter.
///
/// Metadata comes from `gutters.nuon`; this macro provides width + render handlers
/// and creates the inventory linkage.
#[macro_export]
macro_rules! gutter_handler {
	($name:ident, $render:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub(crate) static [<GUTTER_HANDLER_ $name>]: $crate::gutter::handler::GutterHandlerStatic =
				$crate::gutter::handler::GutterHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					handler: $render,
				};

			inventory::submit!($crate::gutter::handler::GutterHandlerReg(&[<GUTTER_HANDLER_ $name>]));
		}
	};
}
