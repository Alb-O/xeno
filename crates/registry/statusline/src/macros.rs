/// Registers a statusline segment in the [`STATUSLINE_SEGMENTS`] slice.
#[macro_export]
macro_rules! statusline_segment {
	($static_name:ident, $name:expr, $position:expr, $priority:expr, $enabled:expr, $render:expr) => {
		#[::linkme::distributed_slice($crate::STATUSLINE_SEGMENTS)]
		static $static_name: $crate::StatuslineSegmentDef = $crate::StatuslineSegmentDef {
			id: $name,
			name: $name,
			position: $position,
			priority: $priority,
			default_enabled: $enabled,
			render: $render,
			source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
		};
	};
}
