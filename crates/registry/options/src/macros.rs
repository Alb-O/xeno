//! Registration macros for options.

/// Selects a provided value or falls back to a default.
#[doc(hidden)]
#[macro_export]
macro_rules! __opt_opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Registers a configuration option in the [`OPTIONS`](crate::OPTIONS) slice.
///
/// # Example
///
/// ```ignore
/// option!(tab_width, Int, 4, Buffer, "Width of a tab character for display");
/// option!(line_numbers, Bool, true, Global, "Show line numbers in the gutter");
/// option!(theme, String, "default".into(), Global, "Current color theme");
/// ```
#[macro_export]
macro_rules! option {
	($name:ident, $type:ident, $default:expr, $scope:ident, $desc:expr $(, priority: $priority:expr)?) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::OPTIONS)]
			static [<OPT_ $name>]: $crate::OptionDef = $crate::OptionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				description: $desc,
				value_type: $crate::OptionType::$type,
				default: || $crate::OptionValue::$type($default),
				scope: $crate::OptionScope::$scope,
				priority: $crate::__opt_opt!($({$priority})?, 0),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}
