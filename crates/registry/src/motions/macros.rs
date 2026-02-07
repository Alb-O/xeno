/// Registers a handler function for a KDL-defined motion.
///
/// The metadata (description, aliases, etc.) comes from `motions.kdl`; this macro
/// only provides the Rust handler and creates the inventory linkage.
#[macro_export]
macro_rules! motion_handler {
	($name:ident, |$text:ident, $range:ident, $count:ident, $extend:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables, non_snake_case)]
			fn [<motion_handler_ $name>](
				$text: ropey::RopeSlice,
				$range: xeno_primitives::Range,
				$count: usize,
				$extend: bool,
			) -> xeno_primitives::Range {
				$body
			}

			#[allow(non_upper_case_globals)]
			pub(crate) static [<MOTION_HANDLER_ $name>]: $crate::motions::MotionHandlerStatic =
				$crate::motions::MotionHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					handler: [<motion_handler_ $name>],
				};

			inventory::submit!($crate::motions::MotionHandlerReg(&[<MOTION_HANDLER_ $name>]));
		}
	};
}
