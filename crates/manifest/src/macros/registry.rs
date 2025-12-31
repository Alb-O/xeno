//! Registry macros for actions, commands, motions, options.
//!
//! These macros register items in [`linkme`] distributed slices.

/// Registers a configuration option in the [`OPTIONS`](crate::options::OPTIONS) slice.
#[macro_export]
macro_rules! option {
	($name:ident, $type:ident, $default:expr, $scope:ident, $desc:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::options::OPTIONS)]
			static [<OPT_ $name>]: $crate::options::OptionDef = $crate::options::OptionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				description: $desc,
				value_type: $crate::options::OptionType::$type,
				default: || $crate::options::OptionValue::$type($default),
				scope: $crate::options::OptionScope::$scope,
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Registers an ex-mode command in the [`COMMANDS`](crate::COMMANDS) slice.
#[macro_export]
macro_rules! command {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::COMMANDS)]
			static [<CMD_ $name>]: $crate::CommandDef = $crate::CommandDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::__opt_slice!($({$aliases})?),
				description: $desc,
				handler: $handler,
				user_data: None,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::__opt_slice!($({$caps})?),
				flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
			};
		}
	};
}

/// Registers a motion primitive in the [`MOTIONS`](crate::motions::MOTIONS) slice.
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
				$range: $crate::Range,
				$count: usize,
				$extend: bool,
			) -> $crate::Range {
				$body
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::motions::MOTIONS)]
			static [<MOTION_ $name>]: $crate::motions::MotionDef = $crate::motions::MotionDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				stringify!($name),
				$crate::__opt_slice!($({$aliases})?),
				$desc,
				$crate::__opt!($({$priority})?, 0),
				$crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				$crate::__opt_slice!($({$caps})?),
				$crate::__opt!($({$flags})?, $crate::flags::NONE),
				[<motion_handler_ $name>],
			);
		}
	};
}

/// Registers a statusline segment in the [`STATUSLINE_SEGMENTS`](crate::STATUSLINE_SEGMENTS) slice.
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

/// Registers a handler for an [`ActionResult`](crate::ActionResult) variant.
#[macro_export]
macro_rules! result_handler {
	($slice:ident, $static_name:ident, $name:literal, $body:expr) => {
		$crate::result_handler!(
			$slice,
			$static_name,
			{
				name: $name
			},
			$body
		);
	};
	(
		$slice:ident,
		$static_name:ident,
		{
			name: $name:literal
			$(, priority: $priority:expr)?
			$(, caps: $caps:expr)?
			$(,)?
		},
		$body:expr
	) => {
		#[::linkme::distributed_slice($crate::actions::$slice)]
		static $static_name: $crate::editor_ctx::ResultHandler =
			$crate::editor_ctx::ResultHandler {
				name: $name,
				priority: $crate::__opt!($({$priority})?, 0),
				required_caps: $crate::__opt_slice!($({$caps})?),
				handle: $body,
			};
	};
}

/// Registers an extension handler for [`ActionResult`](crate::ActionResult).
///
/// Extension handlers run after the core per-variant handlers, in priority order.
/// They should return [`HandleOutcome::NotHandled`](crate::editor_ctx::HandleOutcome::NotHandled)
/// when they don't apply.
#[macro_export]
macro_rules! result_extension_handler {
	($static_name:ident, $name:literal, $body:expr) => {
		$crate::result_extension_handler!(
			$static_name,
			{
				name: $name
			},
			$body
		);
	};
	(
		$static_name:ident,
		{
			name: $name:literal
			$(, priority: $priority:expr)?
			$(, caps: $caps:expr)?
			$(,)?
		},
		$body:expr
	) => {
		#[::linkme::distributed_slice($crate::actions::RESULT_EXTENSION_HANDLERS)]
		static $static_name: $crate::editor_ctx::ResultHandler =
			$crate::editor_ctx::ResultHandler {
				name: $name,
				priority: $crate::__opt!($({$priority})?, 0),
				required_caps: $crate::__opt_slice!($({$caps})?),
				handle: $body,
			};
	};
}
