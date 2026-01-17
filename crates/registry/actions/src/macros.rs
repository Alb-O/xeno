//! Action and result handler registration macros.

/// Defines keybinding lists for an action.
#[doc(hidden)]
#[macro_export]
macro_rules! __action_keybindings {
	($name:ident, $kdl:literal) => {
		xeno_macro::parse_keybindings!($name, $kdl);
		paste::paste! {
			inventory::submit! { $crate::KeyBindingSetReg([<KEYBINDINGS_ $name>]) }
		}
	};
	($name:ident) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<KEYBINDINGS_ $name>]: &'static [$crate::KeyBindingDef] = &[];
		}
	};
}

/// Defines an action definition.
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
///     bindings: r#"normal \"x\" \"ctrl-x\""#,
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
/// [`ActionResult`]: crate::ActionResult
/// [`ResultHandler`]: crate::editor_ctx::ResultHandler
/// [`parse_keybindings!`]: xeno_macro::parse_keybindings
/// [`result_handler!`]: crate::result_handler
/// [`result_extension_handler!`]: crate::result_extension_handler
#[macro_export]
macro_rules! action {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, short_desc: $short:expr)?
		$(, bindings: $kdl:literal)?
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?
	}, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<handler_ $name>]($ctx: &$crate::ActionContext) -> $crate::ActionResult {
				$body
			}

			$crate::action!($name, {
				$(aliases: $aliases,)?
				description: $desc
				$(, short_desc: $short)?
				$(, bindings: $kdl)?
				$(, priority: $priority)?
				$(, caps: $caps)?
				$(, flags: $flags)?
			}, handler: [<handler_ $name>]);
		}
	};

	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, short_desc: $short:expr)?
		$(, bindings: $kdl:literal)?
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<ACTION_ $name>]: $crate::ActionDef = $crate::ActionDef {
				meta: $crate::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: xeno_registry_core::__reg_opt_slice!($({$aliases})?),
					description: $desc,
					priority: xeno_registry_core::__reg_opt!($({$priority})?, 0),
					source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
					required_caps: xeno_registry_core::__reg_opt_slice!($({$caps})?),
					flags: xeno_registry_core::__reg_opt!($({$flags})?, $crate::flags::NONE),
				},
				short_desc: xeno_registry_core::__reg_opt!($({$short})?, ""),
				handler: $handler,
			};

			#[doc = concat!("Typed handle for the `", stringify!($name), "` action.")]
			#[allow(non_upper_case_globals)]
			pub const $name: $crate::ActionKey = $crate::ActionKey::new(&[<ACTION_ $name>]);

			inventory::submit! { $crate::ActionReg(&[<ACTION_ $name>]) }

			$crate::__action_keybindings!($name $(, $kdl)?);
		}
	};
}

/// Defines a handler for an [`ActionResult`](crate::ActionResult) variant.
///
/// Register it explicitly with [`register_result_handler`](crate::register_result_handler).
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
		#[allow(non_upper_case_globals)]
		pub static $static_name: $crate::editor_ctx::ResultHandler =
			$crate::editor_ctx::ResultHandler {
				name: $name,
				priority: xeno_registry_core::__reg_opt!($({$priority})?, 0),
				required_caps: xeno_registry_core::__reg_opt_slice!($({$caps})?),
				handle: $body,
			};
	};
}

/// Registers a key sequence prefix with its description for the which-key HUD.
///
/// Multi-key prefixes require an explicit identifier via the `as` syntax.
///
/// # Example
///
/// ```ignore
/// key_prefix!(normal "g" => "Goto");
/// key_prefix!(normal "ctrl-w f" as ctrl_w_f => "Focus");
/// ```
#[macro_export]
macro_rules! key_prefix {
	($mode:ident $keys:literal => $desc:literal) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<KEY_PREFIX_ $mode:upper _ $keys>]: $crate::KeyPrefixDef = $crate::KeyPrefixDef {
				mode: $crate::BindingMode::[<$mode:camel>],
				keys: $keys,
				description: $desc,
			};
			inventory::submit! { $crate::KeyPrefixReg(&[<KEY_PREFIX_ $mode:upper _ $keys>]) }
		}
	};
	($mode:ident $keys:literal as $id:ident => $desc:literal) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<KEY_PREFIX_ $mode:upper _ $id:upper>]: $crate::KeyPrefixDef = $crate::KeyPrefixDef {
				mode: $crate::BindingMode::[<$mode:camel>],
				keys: $keys,
				description: $desc,
			};
			inventory::submit! { $crate::KeyPrefixReg(&[<KEY_PREFIX_ $mode:upper _ $id:upper>]) }
		}
	};
}

/// Defines an extension handler for [`ActionResult`](crate::ActionResult).
///
/// Register it explicitly with
/// [`register_result_extension_handler`](crate::register_result_extension_handler).
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
		#[allow(non_upper_case_globals)]
		pub static $static_name: $crate::editor_ctx::ResultHandler =
			$crate::editor_ctx::ResultHandler {
				name: $name,
				priority: xeno_registry_core::__reg_opt!($({$priority})?, 0),
				required_caps: xeno_registry_core::__reg_opt_slice!($({$caps})?),
				handle: $body,
			};
	};
}
