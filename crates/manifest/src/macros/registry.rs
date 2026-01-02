//! Registry macros for actions and statusline segments.
//!
//! These macros register items in [`linkme`] distributed slices.
//!
//! Note: `motion!` and `option!` macros have been moved to their respective
//! registry crates (`evildoer-registry-motions` and `evildoer-registry-options`).

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
