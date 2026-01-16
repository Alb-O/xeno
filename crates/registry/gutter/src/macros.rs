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

/// Registers a gutter column in the [`GUTTERS`](crate::GUTTERS) slice.
///
/// # Examples
///
/// ```ignore
/// // Absolute line numbers with dynamic width
/// gutter!(line_numbers, {
///     description: "Absolute line numbers",
///     priority: 0,
///     width: Dynamic(|ctx| (ctx.total_lines.max(1).ilog10() as u16 + 2).max(4)),
/// }, |ctx| {
///     Some(GutterCell {
///         text: format!("{}", ctx.line_idx + 1),
///         style: GutterStyle::Normal,
///     })
/// });
///
/// // Fixed-width sign column
/// gutter!(signs, {
///     description: "Sign column for diagnostics",
///     priority: -10,
///     width: Fixed(2),
///     enabled: true,
/// }, |ctx| {
///     ctx.annotations.sign.map(|c| GutterCell {
///         text: c.to_string(),
///         style: GutterStyle::Normal,
///     })
/// });
///
/// // Disabled by default
/// gutter!(relative_line_numbers, {
///     description: "Relative line numbers",
///     priority: 0,
///     width: Dynamic(|ctx| (ctx.total_lines.max(1).ilog10() as u16 + 2).max(4)),
///     enabled: false,
/// }, |ctx| {
///     let distance = ctx.line_idx.abs_diff(ctx.cursor_line);
///     Some(GutterCell {
///         text: format!("{}", distance),
///         style: GutterStyle::Normal,
///     })
/// });
/// ```
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
			#[linkme::distributed_slice($crate::GUTTERS)]
			static [<GUTTER_ $name:upper>]: $crate::GutterDef = $crate::GutterDef {
				meta: $crate::RegistryMeta {
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
				width: $crate::GutterWidth::$width_kind($width_val),
				render: $render,
			};
		}
	};
}
