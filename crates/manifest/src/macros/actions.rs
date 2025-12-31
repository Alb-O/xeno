//! Action registration macro.
//!
//! The [`action!`] macro registers actions with optional keybindings and result handlers.

/// Registers an action in the [`ACTIONS`](crate::ACTIONS) slice.
///
/// Actions are the primary unit of editor functionality. Each action has a name,
/// description, and handler that returns an [`ActionResult`]. Actions can optionally
/// have keybindings (via `bindings:`) and result handlers (via `result:`).
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
/// **Buffer-ops action** - for actions that delegate to [`BufferOps`] trait methods.
/// Generates the action, keybindings, AND result handler in one declaration:
/// ```ignore
/// action!(split_horizontal, {
///     description: "Split horizontally",
///     bindings: r#"window "s""#,
///     result: SplitHorizontal,
/// }, |ops| ops.split_horizontal());
/// ```
///
/// The `result:` form delegates to [`buffer_ops_handler!`] to generate a
/// [`ResultHandler`] that matches on the specified [`ActionResult`] variant
/// and calls the body with the [`BufferOps`] context.
///
/// [`ActionResult`]: crate::actions::ActionResult
/// [`BufferOps`]: crate::editor_ctx::BufferOps
/// [`ResultHandler`]: crate::editor_ctx::ResultHandler
/// [`parse_keybindings!`]: evildoer_macro::parse_keybindings
/// [`buffer_ops_handler!`]: evildoer_macro::buffer_ops_handler
#[macro_export]
macro_rules! action {
	// Buffer-ops form: action + keybindings + result handler colocated.
	// Delegates handler generation to buffer_ops_handler! proc macro for
	// CamelCase → SCREAMING_SNAKE_CASE slice name derivation.
	($name:ident, {
		description: $desc:expr,
		bindings: $kdl:literal,
		result: $result:ident
		$(,)?
	}, |$ops:ident| $body:expr) => {
		$crate::action!($name, {
			description: $desc,
			bindings: $kdl
		}, |_ctx| $crate::actions::ActionResult::$result);

		evildoer_macro::buffer_ops_handler!($name, $result, $ops, $body);
	};

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
