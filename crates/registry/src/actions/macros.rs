//! Action and result handler registration macros.

/// Defines keybinding lists for an action.
#[doc(hidden)]
#[macro_export]
macro_rules! __action_keybindings {
	($name:ident, bindings: $kdl:literal, action_id: $action_id:expr $(,)?) => {
		xeno_macro::parse_keybindings!($name, $kdl, $action_id);
	};
	($name:ident, action_id: $action_id:expr $(,)?) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<KEYBINDINGS_ $name>]: &'static [$crate::actions::KeyBindingDef] = &[];
		}
	};
}

/// Defines an action definition.
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
			fn [<handler_ $name>]($ctx: &$crate::actions::ActionContext) -> $crate::actions::ActionResult {
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
			pub static [<ACTION_ $name>]: $crate::actions::ActionDef = $crate::actions::ActionDef {
				meta: $crate::actions::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: $crate::__reg_opt_slice!($({$aliases})?),
					description: $desc,
					priority: $crate::__reg_opt!($({$priority})?, 0),
					source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
					required_caps: $crate::__reg_opt_slice!($({$caps})?),
					flags: $crate::__reg_opt!($({$flags})?, $crate::actions::flags::NONE),
				},
				short_desc: $crate::__reg_opt!($({$short})?, ""),
				handler: $handler,
				bindings: [<KEYBINDINGS_ $name>],
			};

			#[doc = concat!("Typed handle for the `", stringify!($name), "` action.")]
			#[allow(non_upper_case_globals)]
			pub const $name: $crate::actions::ActionKey = $crate::actions::ActionKey::new(&[<ACTION_ $name>]);

			$crate::__action_keybindings! {
				$name,
				$(bindings: $kdl,)?
				action_id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
			}
		}
	};
}

/// Defines a handler for an [`ActionResult`](crate::actions::ActionResult) variant.
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
		pub static $static_name: $crate::actions::editor_ctx::ResultHandler =
			$crate::actions::editor_ctx::ResultHandler {
				name: $name,
				priority: $crate::__reg_opt!($({$priority})?, 0),
				required_caps: $crate::__reg_opt_slice!($({$caps})?),
				handle: $body,
			};
	};
}

/// Registers a key sequence prefix with its description for the which-key HUD.
#[macro_export]
macro_rules! key_prefix {
	($mode:ident $keys:literal => $desc:literal) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<KEY_PREFIX_ $mode:upper _ $keys>]: $crate::actions::KeyPrefixDef =
				$crate::actions::KeyPrefixDef {
					mode: $crate::actions::BindingMode::[<$mode:camel>],
					keys: $keys,
					description: $desc,
				};
		}
	};
	($mode:ident $keys:literal as $id:ident => $desc:literal) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<KEY_PREFIX_ $mode:upper _ $id:upper>]: $crate::actions::KeyPrefixDef =
				$crate::actions::KeyPrefixDef {
					mode: $crate::actions::BindingMode::[<$mode:camel>],
					keys: $keys,
					description: $desc,
				};
		}
	};
}

/// Defines an extension handler for [`ActionResult`](crate::actions::ActionResult).
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
		pub static $static_name: $crate::actions::editor_ctx::ResultHandler =
			$crate::actions::editor_ctx::ResultHandler {
				name: $name,
				priority: $crate::__reg_opt!($({$priority})?, 0),
				required_caps: $crate::__reg_opt_slice!($({$caps})?),
				handle: $body,
			};
	};
}
