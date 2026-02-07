//! Statusline segment registration macros.

/// Registers a handler for a KDL-defined statusline segment.
///
/// Metadata comes from `statusline.kdl`; this macro provides the render handler
/// and creates the inventory linkage.
#[macro_export]
macro_rules! segment_handler {
	($name:ident, $render:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub(crate) static [<SEG_HANDLER_ $name>]: $crate::statusline::handler::StatuslineHandlerStatic =
				$crate::statusline::handler::StatuslineHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					render: $render,
				};

			inventory::submit!($crate::statusline::handler::StatuslineHandlerReg(&[<SEG_HANDLER_ $name>]));
		}
	};
}
