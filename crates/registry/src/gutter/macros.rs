//! Registration macros for gutter columns.

/// Helper to select enabled value or default to true.
#[doc(hidden)]
#[macro_export]
macro_rules! __gutter_enabled {
	() => {
		true
	};
	($val:expr) => {
		$val
	};
}

/// Defines a gutter column.
#[macro_export]
macro_rules! gutter {
	($name:ident, {
		description: $desc:expr,
		priority: $priority:expr,
		width: $width_kind:ident($width_val:expr)
		$(, enabled: $enabled:expr)?
	}, $render:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<GUTTER_ $name:upper>]: $crate::gutter::GutterDef = $crate::gutter::GutterDef {
				meta: $crate::gutter::RegistryMetaStatic {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: &[],
					description: $desc,
					priority: $priority,
					source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
					required_caps: &[],
					flags: 0,
				},
				default_enabled: $crate::__gutter_enabled!($($enabled)?),
				width: $crate::gutter::GutterWidth::$width_kind($width_val),
				render: $render,
			};
		}
	};
}
