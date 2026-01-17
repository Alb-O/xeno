//! Statusline segment registration macros.

/// Defines a statusline segment with named parameters.
///
/// # Examples
///
/// ```ignore
/// // Basic segment
/// segment!(mode, {
///     position: Left,
///     description: "Current editor mode",
/// }, |ctx| {
///     Some(RenderedSegment {
///         text: format!(" {} ", ctx.mode_name),
///         style: SegmentStyle::Mode,
///     })
/// });
///
/// // With optional parameters
/// segment!(progress, {
///     position: Right,
///     description: "Scroll position indicator",
///     priority: 20,
///     enabled: true,
/// }, |ctx| { ... });
///
/// // Disabled by default
/// segment!(debug_info, {
///     position: Right,
///     description: "Debug information",
///     enabled: false,
/// }, |ctx| { ... });
/// ```
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
			pub static [<SEG_ $name:upper>]: $crate::StatuslineSegmentDef =
				$crate::StatuslineSegmentDef {
					meta: $crate::RegistryMeta {
						id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
						name: stringify!($name),
						aliases: &[],
						description: $desc,
						priority: xeno_registry_core::__reg_opt!($({$priority})?, 0),
						source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
						required_caps: &[],
						flags: 0,
					},
					position: $crate::SegmentPosition::$position,
					default_enabled: xeno_registry_core::__reg_opt!($({$enabled})?, true),
					render: $render,
				};

			inventory::submit! { $crate::StatuslineSegmentReg(&[<SEG_ $name:upper>]) }
		}
	};
}
