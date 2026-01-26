//! Statusline segment registration macros.

/// Defines a statusline segment with named parameters.
#[macro_export]
macro_rules! segment {
	($name:ident, {
		position: $position:ident,
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, enabled: $enabled:expr)?
		$(,)?
	}, $render:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<SEG_ $name:upper>]: $crate::statusline::StatuslineSegmentDef =
				$crate::statusline::StatuslineSegmentDef {
					meta: $crate::statusline::RegistryMeta {
						id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
						name: stringify!($name),
						aliases: &[],
						description: $desc,
						priority: $crate::xeno_registry_core::__reg_opt!($({$priority})?, 0),
						source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
						required_caps: &[],
						flags: 0,
					},
					position: $crate::statusline::SegmentPosition::$position,
					default_enabled: $crate::xeno_registry_core::__reg_opt!($({$enabled})?, true),
					render: $render,
				};

			inventory::submit! { $crate::inventory::Reg(&[<SEG_ $name:upper>]) }
		}
	};
}
