//! Action registration macro.
//!
//! The [`action!`] macro registers actions with optional keybindings.

/// Registers an action in the [`ACTIONS`](crate::ACTIONS) slice.
///
/// Actions are the primary unit of editor functionality. Each action has a name,
/// description, and handler that returns an [`ActionResult`]. Actions can optionally
/// have keybindings (via `bindings:`). Result handlers are registered separately
/// via [`result_handler!`] or [`result_extension_handler!`].
///
/// # Forms
///
/// **Basic action** - registers handler only, no keybindings:
/// ```ignore
/// action!(name, { description: "..." }, |ctx| { ... });
/// action!(name, { description: "..." }, handler: my_handler_fn);
/// ```
///
/// **Action with keybindings** - parses KDL binding syntax via [`parse_keybindings!`]:
/// ```ignore
/// action!(name, {
///     description: "...",
///     bindings: r#"normal "x" "ctrl-x""#,
/// }, |ctx| { ... });
/// ```
///
/// **Result handlers** - register explicitly with [`result_handler!`] or
/// [`result_extension_handler!`]:
/// ```ignore
/// result_handler!(RESULT_SPLIT_HORIZONTAL_HANDLERS, HANDLE_SPLIT_HORIZONTAL, "split_horizontal", |r, ctx, _| {
///     if let ActionResult::SplitHorizontal = r {
///         if let Some(ops) = ctx.split_ops() {
///             ops.split_horizontal();
///         }
///     }
///     HandleOutcome::Handled
/// });
/// ```
///
/// [`ActionResult`]: crate::actions::ActionResult
/// [`ResultHandler`]: crate::editor_ctx::ResultHandler
/// [`parse_keybindings!`]: evildoer_macro::parse_keybindings
/// [`result_handler!`]: crate::result_handler
/// [`result_extension_handler!`]: crate::result_extension_handler
#[macro_export]
macro_rules! action {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr,
		bindings: $kdl:literal
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?
	}, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<handler_ $name>]($ctx: &$crate::actions::ActionContext) -> $crate::actions::ActionResult {
				$body
			}

			$crate::action!($name, {
				$(aliases: $aliases,)?
				description: $desc,
				bindings: $kdl
				$(, priority: $priority)?
				$(, caps: $caps)?
				$(, flags: $flags)?
			}, handler: [<handler_ $name>]);
		}
	};

	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr,
		bindings: $kdl:literal
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		$crate::action!($name, {
			$(aliases: $aliases,)?
			description: $desc
			$(, priority: $priority)?
			$(, caps: $caps)?
			$(, flags: $flags)?
		}, handler: $handler);
		evildoer_macro::parse_keybindings!($name, $kdl);
	};

	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::ACTIONS)]
			static [<ACTION_ $name>]: $crate::actions::ActionDef = $crate::actions::ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::__opt_slice!($({$aliases})?),
				description: $desc,
				handler: $handler,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: $crate::__opt_slice!($({$caps})?),
				flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
			};
		}
	};

	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?
	}, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<handler_ $name>]($ctx: &$crate::actions::ActionContext) -> $crate::actions::ActionResult {
				$body
			}
			$crate::action!($name, {
				$(aliases: $aliases,)?
				description: $desc
				$(, priority: $priority)?
				$(, caps: $caps)?
				$(, flags: $flags)?
			}, handler: [<handler_ $name>]);
		}
	};
}
