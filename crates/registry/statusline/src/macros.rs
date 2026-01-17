/// Defines a statusline segment definition.
#[macro_export]
macro_rules! statusline_segment {
	($static_name:ident, $name:expr, $position:expr, $priority:expr, $enabled:expr, $render:expr) => {
		pub static $static_name: $crate::StatuslineSegmentDef = $crate::StatuslineSegmentDef {
			meta: $crate::RegistryMeta {
				id: $name,
				name: $name,
				aliases: &[],
				description: "",
				priority: $priority,
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: &[],
				flags: 0,
			},
			position: $position,
			default_enabled: $enabled,
			render: $render,
		};

		inventory::submit! { $crate::StatuslineSegmentReg(&$static_name) }
	};
}
