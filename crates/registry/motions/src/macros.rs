/// Defines a motion primitive.
#[macro_export]
macro_rules! motion {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, |$text:ident, $range:ident, $count:ident, $extend:ident| $body:expr) => {
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
			pub static [<MOTION_ $name>]: $crate::MotionDef = $crate::MotionDef {
				meta: $crate::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: xeno_registry_core::__reg_opt_slice!($({$aliases})?),
					description: $desc,
					priority: xeno_registry_core::__reg_opt!($({$priority})?, 0),
					source: xeno_registry_core::__reg_opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
					required_caps: xeno_registry_core::__reg_opt_slice!($({$caps})?),
					flags: xeno_registry_core::__reg_opt!($({$flags})?, $crate::flags::NONE),
				},
				handler: [<motion_handler_ $name>],
			};

			#[doc = concat!("Typed handle for the `", stringify!($name), "` motion.")]
			#[allow(non_upper_case_globals)]
			pub const $name: $crate::MotionKey = $crate::MotionKey::new(&[<MOTION_ $name>]);

			inventory::submit! { $crate::MotionReg(&[<MOTION_ $name>]) }
		}
	};
}
