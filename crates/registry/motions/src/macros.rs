/// Helper macro for optional values with defaults.
#[doc(hidden)]
#[macro_export]
macro_rules! __motion_opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Helper macro for optional slice values.
#[doc(hidden)]
#[macro_export]
macro_rules! __motion_opt_slice {
	({$val:expr}) => {
		$val
	};
	() => {
		&[]
	};
}

/// Registers a motion primitive in the [`MOTIONS`](crate::MOTIONS) slice.
///
/// # Example
///
/// ```ignore
/// motion!(move_left, { description: "Move left" }, |text, range, count, extend| {
///     // implementation
/// });
/// ```
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
			#[linkme::distributed_slice($crate::MOTIONS)]
			static [<MOTION_ $name>]: $crate::MotionDef = $crate::MotionDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				stringify!($name),
				$crate::__motion_opt_slice!($({$aliases})?),
				$desc,
				$crate::__motion_opt!($({$priority})?, 0),
				$crate::__motion_opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				$crate::__motion_opt_slice!($({$caps})?),
				$crate::__motion_opt!($({$flags})?, $crate::flags::NONE),
				[<motion_handler_ $name>],
			);

			#[doc = concat!("Typed handle for the `", stringify!($name), "` motion.")]
			#[allow(non_upper_case_globals)]
			pub const $name: $crate::MotionKey = $crate::MotionKey::new(&[<MOTION_ $name>]);
		}
	};
}
