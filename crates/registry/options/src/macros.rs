//! Registration macros for options.

/// Selects a provided value or falls back to a default.
#[doc(hidden)]
#[macro_export]
macro_rules! __opt_priority {
	() => {
		0
	};
	($val:expr) => {
		$val
	};
}

/// Registers a configuration option in the [`OPTIONS`](crate::OPTIONS) slice.
///
/// This macro generates:
/// - A static [`OptionDef`](crate::OptionDef) registered in the [`OPTIONS`](crate::OPTIONS) slice
/// - A public constant [`OptionKey`](crate::OptionKey) for type-safe references
///
/// # Example
///
/// ```ignore
/// option!(tab_width, {
///     kdl: "tab-width",
///     type: Int,
///     default: 4,
///     scope: Buffer,
///     description: "Number of spaces a tab character occupies for display",
/// });
///
/// option!(theme, {
///     kdl: "theme",
///     type: String,
///     default: "gruvbox".into(),
///     scope: Global,
///     description: "Active color theme name",
/// });
///
/// // Use the generated typed handle:
/// let def = tab_width.def();
/// let default_value = (def.default)();
/// ```
///
/// The `kdl:` field is the source of truth for config files - what you write
/// is exactly what appears in the KDL configuration.
#[macro_export]
macro_rules! option {
	($name:ident, {
		kdl: $kdl:literal,
		type: $type:ident,
		default: $default:expr,
		scope: $scope:ident,
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::OPTIONS)]
			static [<OPT_ $name:upper>]: $crate::OptionDef = $crate::OptionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				kdl_key: $kdl,
				description: $desc,
				value_type: $crate::OptionType::$type,
				default: || $crate::OptionValue::$type($default),
				scope: $crate::OptionScope::$scope,
				priority: $crate::__opt_priority!($($priority)?),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};

			#[doc = concat!("Typed handle for the `", stringify!($name), "` option.")]
			#[allow(non_upper_case_globals)]
			pub const $name: $crate::OptionKey = $crate::OptionKey::new(&[<OPT_ $name:upper>]);
		}
	};
}
