//! Registry macros for actions, commands, statusline segments.
//!
//! These macros register items in [`linkme`] distributed slices.
//!
//! Note: `motion!` and `option!` macros have been moved to their respective
//! registry crates (`evildoer-registry-motions` and `evildoer-registry-options`).

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
