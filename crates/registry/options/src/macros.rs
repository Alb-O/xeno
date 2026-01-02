//! Registration macros for options.

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
	($name:ident, $type:ident, $default:expr, $scope:ident, $desc:expr) => {
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
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}
