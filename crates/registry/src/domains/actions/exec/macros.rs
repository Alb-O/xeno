//! Action and result handler registration macros.

/// Registers a handler function for a registry-defined action.
///
/// The metadata (description, bindings, etc.) comes from `actions.nuon`; this macro
/// only provides the Rust handler and creates the `ActionKey` typed handle.
#[macro_export]
macro_rules! action_handler {
	($name:ident, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<handler_ $name>]($ctx: &$crate::actions::ActionContext) -> $crate::actions::ActionResult {
				$body
			}

			$crate::action_handler!($name, handler: [<handler_ $name>]);
		}
	};
	($name:ident, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub(crate) static [<HANDLER_ $name>]: $crate::actions::ActionHandlerStatic =
				$crate::actions::ActionHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					handler: $handler,
				};

			inventory::submit!($crate::actions::ActionHandlerReg(&[<HANDLER_ $name>]));

			#[doc = concat!("Typed handle for the `", stringify!($name), "` action.")]
			#[allow(non_upper_case_globals)]
			pub const $name: $crate::actions::ActionKey =
				$crate::actions::ActionKey::new(concat!("xeno-registry::", stringify!($name)));
		}
	};
}
